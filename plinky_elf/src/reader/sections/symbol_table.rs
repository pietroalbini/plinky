use crate::errors::LoadError;
use crate::ids::{ElfSectionId, ElfStringId, ElfSymbolId};
use crate::raw::RawSymbol;
use crate::reader::sections::reader::{SectionMetadata, SectionReader};
use crate::reader::ReadCursor;
use crate::{
    ElfSymbol, ElfSymbolBinding, ElfSymbolDefinition, ElfSymbolTable, ElfSymbolType,
    ElfSymbolVisibility,
};
use std::collections::BTreeMap;

pub(crate) fn read(
    reader: &mut SectionReader<'_, '_>,
    meta: &dyn SectionMetadata,
    dynsym: bool,
) -> Result<ElfSymbolTable, LoadError> {
    let mut symbols = BTreeMap::new();
    for cursor in reader.entries()? {
        symbols.insert(
            ElfSymbolId { section: meta.section_id(), index: symbols.len() as _ },
            read_symbol(meta, cursor)?,
        );
    }

    Ok(ElfSymbolTable { dynsym, symbols })
}

fn read_symbol(
    meta: &dyn SectionMetadata,
    mut cursor: ReadCursor<'_>,
) -> Result<ElfSymbol, LoadError> {
    let symbol: RawSymbol = cursor.read_raw()?;
    Ok(ElfSymbol {
        name: ElfStringId { section: meta.section_link(), offset: symbol.name_offset },
        binding: match (symbol.info & 0b11110000) >> 4 {
            0 => ElfSymbolBinding::Local,
            1 => ElfSymbolBinding::Global,
            2 => ElfSymbolBinding::Weak,
            other => ElfSymbolBinding::Unknown(other),
        },
        type_: match symbol.info & 0b1111 {
            0 => ElfSymbolType::NoType,
            1 => ElfSymbolType::Object,
            2 => ElfSymbolType::Function,
            3 => ElfSymbolType::Section,
            4 => ElfSymbolType::File,
            other => ElfSymbolType::Unknown(other),
        },
        visibility: match symbol.other {
            0 => ElfSymbolVisibility::Default,
            1 | 2 => ElfSymbolVisibility::Hidden,
            3 => ElfSymbolVisibility::Protected,
            4 => ElfSymbolVisibility::Exported,
            5 => ElfSymbolVisibility::Singleton,
            6 => ElfSymbolVisibility::Eliminate,
            other => return Err(LoadError::BadSymbolVisibility(other)),
        },
        definition: match symbol.definition {
            0x0000 => ElfSymbolDefinition::Undefined, // SHN_UNDEF
            0xFFF1 => ElfSymbolDefinition::Absolute,  // SHN_ABS
            0xFFF2 => ElfSymbolDefinition::Common,    // SHN_COMMON
            other => ElfSymbolDefinition::Section(ElfSectionId { index: other as _ }),
        },
        value: symbol.value,
        size: symbol.size,
    })
}
