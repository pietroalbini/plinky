use crate::ids::ElfSectionId;
use crate::raw::{RawGnuHashHeader, RawHashHeader};
use crate::reader::sections::{
    SectionMetadata, SectionReader, dynamic, string_table, symbol_table,
};
use crate::{
    ElfDynamicDirective, ElfReader, ElfSegment, ElfSegmentType, ElfStringTable, ElfSymbolBinding,
    ElfSymbolDefinition, ElfSymbolVisibility, LoadError,
};
use plinky_macros::{Display, Error};
use plinky_utils::raw_types::{RawType, RawTypeAsPointerSize};

pub struct ElfDynamicReader<'reader, 'src> {
    reader: &'reader mut ElfReader<'src>,
    dynamic: DynamicSegment,

    symbols_count: Option<u32>,
    symbols: Option<Vec<ElfSymbolInDynamic>>,
    strings: Option<ElfStringTable>,
    needed_libraries: Option<Vec<String>>,
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
            symbols: None,
            needed_libraries: None,
        })
    }

    pub fn soname(&mut self) -> Result<Option<String>, ReadDynamicError> {
        let Some(offset) = self.dynamic.soname.value else { return Ok(None) };
        Ok(Some(
            self.get_string(
                u32::try_from(offset)
                    .map_err(|_| ReadDynamicError::StringOffsetTooLarge(offset))?,
            )?
            .to_string(),
        ))
    }

    pub fn needed_libraries(&mut self) -> Result<&[String], ReadDynamicError> {
        if self.dynamic.needed.is_empty() {
            return Ok(&[]);
        }

        if self.needed_libraries.is_none() {
            let mut needed = Vec::new();
            for offset in self.dynamic.needed.clone() {
                needed.push(self.get_string(offset as u32)?.to_string());
            }
            self.needed_libraries = Some(needed);
        }

        Ok(self.needed_libraries.as_ref().unwrap())
    }

    pub fn has_symbol(&mut self, name: &str) -> Result<bool, ReadDynamicError> {
        // TODO: implement this by looking into the hash map
        let symbols = self.symbols()?;
        for symbol in symbols {
            if symbol.name == name {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub fn symbols(&mut self) -> Result<&[ElfSymbolInDynamic], ReadDynamicError> {
        if self.symbols.is_none() {
            let content_entry_len = self.dynamic.symbol_entry_size.get()?;
            let mut reader = SectionReader {
                content_len: u64::from(self.symbols_count()?) * content_entry_len,
                content_start: self.dynamic.symbol_addr.get()?,
                content_entry_len,
                parent_cursor: &mut self.reader.cursor,
            };
            let symbols = symbol_table::read(&mut reader, &FakeMetadata, true)
                .map_err(ReadDynamicError::InvalidSymbolTable)?;

            let mut result = Vec::new();
            for symbol in symbols.symbols.into_values() {
                result.push(ElfSymbolInDynamic {
                    name: self.get_string(symbol.name.offset)?.to_string(),
                    visibility: symbol.visibility,
                    binding: symbol.binding,
                    defined: symbol.definition != ElfSymbolDefinition::Undefined,
                });
            }
            self.symbols = Some(result);
        }
        Ok(self.symbols.as_ref().unwrap())
    }

    fn symbols_count(&mut self) -> Result<u32, ReadDynamicError> {
        if let Some(count) = self.symbols_count {
            return Ok(count);
        }

        // The ELF specification doesn't include a dynamic directive to list the size of the
        // symbol table, nor the number of symbols. The only way to get the information is to
        // parse DT_HASH or DT_GNU_HASH and extract the symbol count from it.
        let count = if let Some(hash_addr) = self.dynamic.hash_addr.value {
            // Parsing the symbol count from DT_GNU_HASH is trickier, so try DT_HASH first.
            self.parse_symbol_count_from_hash(hash_addr)
                .map_err(ReadDynamicError::InvalidHashSection)?
        } else if let Some(gnu_hash_addr) = self.dynamic.gnu_hash_addr.value {
            self.parse_symbol_count_from_gnu_hash(gnu_hash_addr)
                .map_err(ReadDynamicError::InvalidGnuHashTable)?
        } else {
            return Err(ReadDynamicError::NoHashTables);
        };

        self.symbols_count = Some(count);
        Ok(count)
    }

    fn parse_symbol_count_from_hash(&mut self, hash_addr: u64) -> Result<u32, LoadError> {
        self.reader.cursor.seek_to(hash_addr)?;
        let raw = self.reader.cursor.read_raw::<RawHashHeader>()?;
        Ok(raw.chain_count)
    }

    fn parse_symbol_count_from_gnu_hash(&mut self, gnu_hash_addr: u64) -> Result<u32, LoadError> {
        self.reader.cursor.seek_to(gnu_hash_addr)?;
        let header = self.reader.cursor.read_raw::<RawGnuHashHeader>()?;

        let bits = self.reader.cursor.class;
        self.reader
            .cursor
            .skip(<u64 as RawTypeAsPointerSize>::size(bits) as u64 * header.bloom_count as u64)?;

        let mut max_chain = None;
        for _ in 0..header.buckets_count {
            let chain: u32 = self.reader.cursor.read_raw()?;
            if chain < header.symbols_offset {
                continue;
            }

            match max_chain {
                None => max_chain = Some(chain),
                Some(existing) => max_chain = Some(existing.max(chain)),
            }
        }
        let Some(max_chain) = max_chain else { return Ok(header.symbols_offset) };

        self.reader
            .cursor
            .skip((max_chain - header.symbols_offset) as u64 * u32::size(bits) as u64)?;
        let mut symbols_count = max_chain;
        loop {
            symbols_count += 1;
            if self.reader.cursor.read_raw::<u32>()? & 1 == 1 {
                break;
            }
        }

        Ok(symbols_count)
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

pub struct ElfSymbolInDynamic {
    pub name: String,
    pub visibility: ElfSymbolVisibility,
    pub binding: ElfSymbolBinding,
    pub defined: bool,
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
            ElfDynamicDirective::Needed { string_table_offset } => {
                parsed.needed.push(string_table_offset)
            }
            ElfDynamicDirective::Hash { address } => parsed.hash_addr.set(address)?,
            ElfDynamicDirective::GnuHash { address } => parsed.gnu_hash_addr.set(address)?,
            ElfDynamicDirective::StringTable { address } => parsed.string_addr.set(address)?,
            ElfDynamicDirective::StringTableSize { bytes } => parsed.string_size.set(bytes)?,
            ElfDynamicDirective::SymbolTable { address } => parsed.symbol_addr.set(address)?,
            ElfDynamicDirective::SharedObjectName { string_table_offset } => {
                parsed.soname.set(string_table_offset)?
            }
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
    needed: Vec<u64>,
    soname: DynamicDirective<DT_SONAME>,
    hash_addr: DynamicDirective<DT_HASH>,
    gnu_hash_addr: DynamicDirective<DT_GNU_HASH>,
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

directive_names![DT_HASH, DT_GNU_HASH, DT_STRTAB, DT_STRSZ, DT_SYMTAB, DT_SYMENT, DT_SONAME];

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
    #[display("invalid GNU hash table")]
    InvalidGnuHashTable(#[source] LoadError),
    #[display("invalid symbol table")]
    InvalidSymbolTable(#[source] LoadError),
    #[display("missing string at offset {f0}")]
    MissingString(u32),
    #[display("string offset {f0} too large")]
    StringOffsetTooLarge(u64),
    #[display("no hash tables are present in the object file")]
    NoHashTables,
}
