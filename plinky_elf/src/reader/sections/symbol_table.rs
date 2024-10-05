use crate::errors::LoadError;
use crate::ids::{ElfSectionId, ElfStringId, ElfSymbolId};
use crate::raw::RawSymbol;
use crate::reader::ReadCursor;
use crate::{
    ElfSectionContent, ElfSymbol, ElfSymbolBinding, ElfSymbolDefinition, ElfSymbolTable,
    ElfSymbolType, ElfSymbolVisibility,
};
use std::collections::BTreeMap;

pub(super) fn read(
    cursor: &mut ReadCursor<'_>,
    raw_content: &[u8],
    strings_table: ElfSectionId,
    current_section: ElfSectionId,
    dynsym: bool,
) -> Result<ElfSectionContent, LoadError> {
    let mut inner = std::io::Cursor::new(raw_content);
    let mut cursor = cursor.duplicate(&mut inner);

    let mut symbols = BTreeMap::new();
    while cursor.current_position()? != raw_content.len() as u64 {
        symbols.insert(
            ElfSymbolId { section: current_section, index: symbols.len() as _ },
            read_symbol(&mut cursor, strings_table)?,
        );
    }

    Ok(ElfSectionContent::SymbolTable(ElfSymbolTable { dynsym, symbols }))
}

fn read_symbol(
    cursor: &mut ReadCursor<'_>,
    strings_table: ElfSectionId,
) -> Result<ElfSymbol, LoadError> {
    let symbol: RawSymbol = cursor.read_raw()?;
    Ok(ElfSymbol {
        name: ElfStringId { section: strings_table, offset: symbol.name_offset },
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
