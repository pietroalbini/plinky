use crate::ids::ElfSectionId;
use crate::raw::RawHashHeader;
use crate::reader::sections::{
    dynamic, string_table, symbol_table, SectionMetadata, SectionReader,
};
use crate::{
    ElfDynamicDirective, ElfReader, ElfSegment, ElfSegmentType, ElfStringTable, LoadError,
};
use plinky_macros::{Display, Error};

pub struct ElfDynamicReader<'reader, 'src> {
    reader: &'reader mut ElfReader<'src>,
    dynamic: DynamicSegment,

    symbols_count: Option<u32>,
    symbol_names: Option<Vec<String>>,
    strings: Option<ElfStringTable>,
}

impl<'reader, 'src> ElfDynamicReader<'reader, 'src> {
    pub(super) fn new(reader: &'reader mut ElfReader<'src>) -> Result<Self, ReadDynamicError> {
        let segment = find_dynamic_segment(&reader.segments)?;
        let dynamic = parse_dynamic_segment(reader, &segment)?;

        Ok(ElfDynamicReader {
            reader,
            dynamic,
            symbols_count: None,
            strings: None,
            symbol_names: None,
        })
    }

    pub fn symbol_names(&mut self) -> Result<&[String], ReadDynamicError> {
        if self.symbol_names.is_none() {
            let content_entry_len = self.dynamic.symbol_entry_size.get()?;
            let mut reader = SectionReader {
                content_len: u64::from(self.symbols_count()?) * content_entry_len,
                content_start: self.dynamic.symbol_addr.get()?,
                content_entry_len,
                parent_cursor: &mut self.reader.cursor,
            };
            let symbols = symbol_table::read(&mut reader, &FakeMetadata, true)
                .map_err(ReadDynamicError::InvalidSymbolTable)?;

            let mut names = Vec::new();
            for symbol in symbols.symbols.into_values() {
                names.push(self.get_string(symbol.name.offset)?.to_string());
            }
            self.symbol_names = Some(names);
        }
        Ok(self.symbol_names.as_ref().unwrap())
    }

    fn symbols_count(&mut self) -> Result<u32, ReadDynamicError> {
        if let Some(count) = self.symbols_count {
            Ok(count)
        } else {
            let hash_addr = self.dynamic.hash_addr.get()?;

            // The ELF specification doesn't include a dynamic directive to list the size of the
            // symbol table, nor the number of symbols. The only way to get the information is to
            // read the number of chain items in DT_HASH, which corresponds to the symbols count.
            //
            // Note that some object files might only have DT_GNU_HASH and not DT_HASH, and that is
            // currently not supported. To implement support, see this article:
            //
            //     https://maskray.me/blog/2022-08-21-glibc-and-dt-gnu-hash#dt_symtabsz-or-dt_symtab_count
            //
            self.reader.cursor.seek_to(hash_addr).map_err(ReadDynamicError::InvalidHashSection)?;
            let raw = self
                .reader
                .cursor
                .read_raw::<RawHashHeader>()
                .map_err(ReadDynamicError::InvalidHashSection)?;

            self.symbols_count = Some(raw.chain_count);
            Ok(raw.chain_count)
        }
    }

    fn get_string(&mut self, offset: u32) -> Result<&str, ReadDynamicError> {
        if self.strings.is_none() {
            let mut reader = SectionReader {
                parent_cursor: &mut self.reader.cursor,
                content_len: self.dynamic.string_size.get()?,
                content_start: self.dynamic.string_addr.get()?,
                content_entry_len: 0,
            };
            self.strings = Some(
                string_table::read(&mut reader).map_err(ReadDynamicError::InvalidStringTable)?,
            );
        }
        let table = self.strings.as_ref().unwrap();
        table.get(offset).ok_or(ReadDynamicError::MissingString(offset))
    }
}

fn find_dynamic_segment(segments: &[ElfSegment]) -> Result<ElfSegment, ReadDynamicError> {
    let mut found = None;
    for segment in segments {
        let ElfSegmentType::Dynamic = segment.type_ else { continue };
        match found {
            Some(_) => return Err(ReadDynamicError::DuplicateSegment),
            None => found = Some(segment),
        }
    }
    found.cloned().ok_or(ReadDynamicError::MissingSegment)
}

fn parse_dynamic_segment(
    reader: &mut ElfReader<'_>,
    segment: &ElfSegment,
) -> Result<DynamicSegment, ReadDynamicError> {
    let mut parsed = DynamicSegment::default();

    let mut reader = SectionReader {
        parent_cursor: &mut reader.cursor,
        content_len: segment.file_size,
        content_start: segment.file_offset,
        content_entry_len: 0,
    };

    let directives =
        dynamic::read_directives(&mut reader).map_err(ReadDynamicError::ReadDirectives)?;
    for directive in directives {
        match directive {
            ElfDynamicDirective::Hash { address } => parsed.hash_addr.set(address)?,
            ElfDynamicDirective::StringTable { address } => parsed.string_addr.set(address)?,
            ElfDynamicDirective::StringTableSize { bytes } => parsed.string_size.set(bytes)?,
            ElfDynamicDirective::SymbolTable { address } => parsed.symbol_addr.set(address)?,
            ElfDynamicDirective::SymbolTableEntrySize { bytes } => {
                parsed.symbol_entry_size.set(bytes)?
            }
            _ => {}
        }
    }

    Ok(parsed)
}

#[derive(Debug, Default)]
struct DynamicSegment {
    hash_addr: DynamicDirective<DT_HASH>,
    string_addr: DynamicDirective<DT_STRTAB>,
    string_size: DynamicDirective<DT_STRSZ>,
    symbol_addr: DynamicDirective<DT_SYMTAB>,
    symbol_entry_size: DynamicDirective<DT_SYMENT>,
}

#[derive(Debug, Default)]
struct DynamicDirective<N: DirectiveName> {
    _name: N,
    value: Option<u64>,
}

impl<N: DirectiveName> DynamicDirective<N> {
    fn get(&self) -> Result<u64, ReadDynamicError> {
        self.value.ok_or(ReadDynamicError::MissingDirective(N::NAME))
    }

    fn set(&mut self, value: u64) -> Result<(), ReadDynamicError> {
        if self.value.is_some() {
            return Err(ReadDynamicError::DuplicateDirective(N::NAME));
        }
        self.value = Some(value);
        Ok(())
    }
}

trait DirectiveName {
    const NAME: &'static str;
}

macro_rules! directive_names {
    ($($name:ident),*$(,)?) => {
        $(
            #[derive(Debug, Default)]
            #[allow(non_camel_case_types)]
            struct $name;
            impl DirectiveName for $name {
                const NAME: &'static str = stringify!($name);
            }
        )*
    }
}

directive_names![DT_HASH, DT_STRTAB, DT_STRSZ, DT_SYMTAB, DT_SYMENT];

struct FakeMetadata;

impl SectionMetadata for FakeMetadata {
    fn info_field(&self) -> u32 {
        unimplemented!();
    }

    fn section_id(&self) -> crate::ids::ElfSectionId {
        ElfSectionId { index: u32::MAX }
    }

    fn section_link(&self) -> crate::ids::ElfSectionId {
        ElfSectionId { index: u32::MAX }
    }

    fn section_info(&self) -> crate::ids::ElfSectionId {
        unimplemented!();
    }

    fn permissions(&self) -> crate::ElfPermissions {
        unimplemented!();
    }

    fn deduplication_flag(&self) -> Result<crate::ElfDeduplication, LoadError> {
        unimplemented!();
    }
}

#[derive(Debug, Error, Display)]
pub enum ReadDynamicError {
    #[display("missing dynamic segment")]
    MissingSegment,
    #[display("duplicate dynamic segment")]
    DuplicateSegment,
    #[display("failed to read dynamic directives")]
    ReadDirectives(#[source] LoadError),
    #[display("duplicate dynamic directive {f0}")]
    DuplicateDirective(&'static str),
    #[display("missing dynamic directive {f0}")]
    MissingDirective(&'static str),
    #[display("invalid hash section")]
    InvalidHashSection(#[source] LoadError),
    #[display("invalid string table")]
    InvalidStringTable(#[source] LoadError),
    #[display("invalid symbol table")]
    InvalidSymbolTable(#[source] LoadError),
    #[display("missing string at offset {f0}")]
    MissingString(u32),
}
