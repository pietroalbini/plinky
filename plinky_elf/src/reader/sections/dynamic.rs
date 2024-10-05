use crate::errors::LoadError;
use crate::ids::ElfSectionId;
use crate::raw::RawSectionHeader;
use crate::reader::ReadCursor;
use crate::{
    ElfClass, ElfDynamic, ElfDynamicDirective, ElfDynamicFlags, ElfDynamicFlags1,
    ElfPLTRelocationsMode, ElfSectionContent,
};
use plinky_utils::bitfields::Bitfield;

pub(super) fn read(
    header: &RawSectionHeader,
    raw_content: &[u8],
    cursor: &mut ReadCursor,
) -> Result<ElfSectionContent, LoadError> {
    let mut inner = std::io::Cursor::new(raw_content);
    let mut cursor = cursor.duplicate(&mut inner);

    let mut directives = Vec::new();
    let mut stop = false;
    while !stop {
        let (tag, value): (u64, u64) = match cursor.class {
            ElfClass::Elf32 => (cursor.read_raw::<i32>()? as _, cursor.read_raw::<u32>()? as _),
            ElfClass::Elf64 => (cursor.read_raw()?, cursor.read_raw()?),
        };
        directives.push(match tag {
            0 => {
                stop = true;
                ElfDynamicDirective::Null
            }
            1 => ElfDynamicDirective::Needed { string_table_offset: value },
            2 => ElfDynamicDirective::PLTRelocationsSize { bytes: value },
            3 => ElfDynamicDirective::PLTGOT { address: value },
            4 => ElfDynamicDirective::Hash { address: value },
            5 => ElfDynamicDirective::StringTable { address: value },
            6 => ElfDynamicDirective::SymbolTable { address: value },
            7 => ElfDynamicDirective::Rela { address: value },
            8 => ElfDynamicDirective::RelaSize { bytes: value },
            9 => ElfDynamicDirective::RelaEntrySize { bytes: value },
            10 => ElfDynamicDirective::StringTableSize { bytes: value },
            11 => ElfDynamicDirective::SymbolTableEntrySize { bytes: value },
            12 => ElfDynamicDirective::InitFunction { address: value },
            13 => ElfDynamicDirective::FiniFunction { address: value },
            14 => ElfDynamicDirective::SharedObjectName { string_table_offset: value },
            15 => ElfDynamicDirective::RuntimePath { string_table_offset: value },
            16 => ElfDynamicDirective::Symbolic,
            17 => ElfDynamicDirective::Rel { address: value },
            18 => ElfDynamicDirective::RelSize { bytes: value },
            19 => ElfDynamicDirective::RelEntrySize { bytes: value },
            20 => ElfDynamicDirective::PTLRelocationsMode {
                mode: match value {
                    7 => ElfPLTRelocationsMode::Rela,
                    17 => ElfPLTRelocationsMode::Rel,
                    other => ElfPLTRelocationsMode::Unknown(other),
                },
            },
            21 => ElfDynamicDirective::Debug { address: value },
            22 => ElfDynamicDirective::RelocationsWillModifyText,
            23 => ElfDynamicDirective::JumpRel { address: value },
            24 => ElfDynamicDirective::BindNow,
            30 => ElfDynamicDirective::Flags(
                ElfDynamicFlags::read(value).map_err(LoadError::DynamicFlags)?,
            ),
            0x6ffffef5 => ElfDynamicDirective::GnuHash { address: value },
            0x6ffffffb => ElfDynamicDirective::Flags1(
                ElfDynamicFlags1::read(value).map_err(LoadError::DynamicFlags1)?,
            ),
            _ => ElfDynamicDirective::Unknown { tag, value },
        });
    }

    Ok(ElfSectionContent::Dynamic(ElfDynamic {
        string_table: ElfSectionId { index: header.link },
        directives,
    }))
}
