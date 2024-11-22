use crate::errors::LoadError;
use crate::ids::{ElfSectionId, ElfStringId, ElfSymbolId};
use crate::raw::RawSymbol;
use crate::reader::ReadCursor;
use crate::{
    ElfSectionContent, ElfSymbol, ElfSymbolBinding, ElfSymbolDefinition, ElfSymbolTable,
    ElfSymbolType, ElfSymbolVisibility,
};
use std::collections::BTreeMap;
use crate::reader::sections::reader::{SectionMetadata, SectionReader};

pub(super) fn read(
    reader: &mut SectionReader<'_, '_>,
    meta: &dyn SectionMetadata,
    dynsym: bool,
) -> Result<ElfSectionContent, LoadError> {
    let mut cursor = reader.content_cursor()?;

    let mut symbols = BTreeMap::new();
    while cursor.current_position()? != reader.content_len {
        symbols.insert(
            ElfSymbolId { section: meta.section_id(), index: symbols.len() as _ },
            read_symbol(meta, &mut cursor)?,
        );
    }

    Ok(ElfSectionContent::SymbolTable(ElfSymbolTable { dynsym, symbols }))
}

fn read_symbol(
    meta: &dyn SectionMetadata,
    cursor: &mut ReadCursor<'_>,
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
