use super::{PendingStringId, PendingSymbolId};
use crate::errors::LoadError;
use crate::reader::notes::read_notes;
use crate::reader::{Cursor, PendingIds, PendingSectionId};
use crate::{
    ElfClass, ElfProgramSection, ElfRelocation, ElfRelocationType, ElfRelocationsTable, ElfSection,
    ElfSectionContent, ElfStringTable, ElfSymbol, ElfSymbolBinding, ElfSymbolDefinition,
    ElfSymbolTable, ElfSymbolType, ElfUnknownSection, RawBytes,
};
use std::collections::BTreeMap;

pub(super) fn read_sections(
    cursor: &mut Cursor<'_>,
    offset: u64,
    count: u16,
    size: u16,
    section_names_table: PendingSectionId,
) -> Result<BTreeMap<PendingSectionId, ElfSection<PendingIds>>, LoadError> {
    if offset == 0 {
        return Ok(BTreeMap::new());
    }

    let mut sections = BTreeMap::new();
    for idx in 0..count {
        cursor.seek_to(offset + (size as u64 * idx as u64))?;
        sections.insert(
            PendingSectionId(idx as _),
            read_section(cursor, section_names_table, PendingSectionId(idx as _))?,
        );
    }

    Ok(sections)
}

fn read_section(
    cursor: &mut Cursor<'_>,
    section_names_table: PendingSectionId,
    current_section: PendingSectionId,
) -> Result<ElfSection<PendingIds>, LoadError> {
    let name_offset = cursor.read_u32()?;
    let type_ = cursor.read_u32()?;
    let flags = cursor.read_usize()?;
    let memory_address = cursor.read_usize()?;
    let offset = cursor.read_usize()?;
    let size = cursor.read_usize()?;
    let link = cursor.read_u32()?;
    let info = cursor.read_u32()?;
    let _addr_align = cursor.read_usize()?;
    let _entries_size = cursor.read_usize()?;

    cursor.seek_to(offset)?;
    let raw_content = cursor.read_vec(size)?;
    let content = match type_ {
        0 => ElfSectionContent::Null,
        1 => ElfSectionContent::Program(ElfProgramSection {
            writeable: flags & 0x1 > 0,
            allocated: flags & 0x2 > 0,
            executable: flags & 0x4 > 0,
            raw: RawBytes(raw_content),
        }),
        2 => read_symbol_table(
            cursor,
            &raw_content,
            PendingSectionId(link),
            current_section,
        )?,
        3 => read_string_table(&raw_content)?,
        4 => read_relocations_table(
            cursor,
            &raw_content,
            PendingSectionId(link),
            PendingSectionId(info),
            true,
        )?,
        7 => ElfSectionContent::Note(read_notes(cursor, &raw_content)?),
        9 => read_relocations_table(
            cursor,
            &raw_content,
            PendingSectionId(link),
            PendingSectionId(info),
            false,
        )?,
        other => ElfSectionContent::Unknown(ElfUnknownSection {
            id: other,
            raw: RawBytes(raw_content),
        }),
    };

    Ok(ElfSection {
        name: PendingStringId(section_names_table, name_offset),
        memory_address,
        content,
    })
}

fn read_string_table(raw_content: &[u8]) -> Result<ElfSectionContent<PendingIds>, LoadError> {
    let mut strings = BTreeMap::new();
    let mut offset: usize = 0;
    while offset < raw_content.len() {
        let terminator = raw_content
            .iter()
            .skip(offset as _)
            .position(|&byte| byte == 0)
            .ok_or(LoadError::UnterminatedString)?;
        strings.insert(
            offset as u32,
            String::from_utf8(raw_content[offset..(offset + terminator)].to_vec())?,
        );

        offset += terminator + 1;
    }
    Ok(ElfSectionContent::StringTable(ElfStringTable::new(strings)))
}

fn read_symbol_table(
    cursor: &mut Cursor<'_>,
    raw_content: &[u8],
    strings_table: PendingSectionId,
    current_section: PendingSectionId,
) -> Result<ElfSectionContent<PendingIds>, LoadError> {
    let mut inner = std::io::Cursor::new(raw_content);
    let mut cursor = cursor.duplicate(&mut inner);

    let mut symbols = BTreeMap::new();
    while cursor.current_position()? != raw_content.len() as u64 {
        symbols.insert(
            PendingSymbolId(current_section, symbols.len() as _),
            read_symbol(&mut cursor, strings_table)?,
        );
    }

    Ok(ElfSectionContent::SymbolTable(ElfSymbolTable { symbols }))
}

fn read_symbol(
    cursor: &mut Cursor<'_>,
    strings_table: PendingSectionId,
) -> Result<ElfSymbol<PendingIds>, LoadError> {
    let mut value = 0;
    let mut size = 0;

    let name_offset = cursor.read_u32()?;
    if let Some(ElfClass::Elf32) = cursor.class {
        value = cursor.read_usize()?;
        size = cursor.read_usize()?;
    }
    let info = cursor.read_u8()?;
    let _ = cursor.read_u8()?; // Reserved
    let definition = cursor.read_u16()?;
    if let Some(ElfClass::Elf64) = cursor.class {
        value = cursor.read_usize()?;
        size = cursor.read_usize()?;
    }

    Ok(ElfSymbol {
        name: PendingStringId(strings_table, name_offset),
        binding: match (info & 0b11110000) >> 4 {
            0 => ElfSymbolBinding::Local,
            1 => ElfSymbolBinding::Global,
            2 => ElfSymbolBinding::Weak,
            other => ElfSymbolBinding::Unknown(other),
        },
        type_: match info & 0b1111 {
            0 => ElfSymbolType::NoType,
            1 => ElfSymbolType::Object,
            2 => ElfSymbolType::Function,
            3 => ElfSymbolType::Section,
            4 => ElfSymbolType::File,
            other => ElfSymbolType::Unknown(other),
        },
        definition: match definition {
            0x0000 => ElfSymbolDefinition::Undefined, // SHN_UNDEF
            0xFFF1 => ElfSymbolDefinition::Absolute,  // SHN_ABS
            0xFFF2 => ElfSymbolDefinition::Common,    // SHN_COMMON
            other => ElfSymbolDefinition::Section(PendingSectionId(other as _)),
        },
        value,
        size,
    })
}

fn read_relocations_table(
    cursor: &mut Cursor<'_>,
    raw_content: &[u8],
    symbol_table: PendingSectionId,
    applies_to_section: PendingSectionId,
    rela: bool,
) -> Result<ElfSectionContent<PendingIds>, LoadError> {
    let mut inner = std::io::Cursor::new(raw_content);
    let mut cursor = cursor.duplicate(&mut inner);

    let mut relocations = Vec::new();
    while cursor.current_position()? != raw_content.len() as u64 {
        relocations.push(read_relocation(&mut cursor, symbol_table, rela)?);
    }

    Ok(ElfSectionContent::RelocationsTable(ElfRelocationsTable {
        symbol_table,
        applies_to_section,
        relocations,
    }))
}

fn read_relocation(
    cursor: &mut Cursor<'_>,
    symbol_table: PendingSectionId,
    rela: bool,
) -> Result<ElfRelocation<PendingIds>, LoadError> {
    let offset = cursor.read_usize()?;
    let info = cursor.read_usize()?;
    let (symbol, relocation_type) = match cursor.class {
        Some(ElfClass::Elf32) => (
            (info >> 8) as u32,
            match info & 0xF {
                0 => ElfRelocationType::X86_None,
                1 => ElfRelocationType::X86_32,
                2 => ElfRelocationType::X86_PC32,
                other => ElfRelocationType::Unknown(other as _),
            },
        ),
        Some(ElfClass::Elf64) => (
            (info >> 32) as u32,
            match info & 0xFFFF_FFFF {
                0 => ElfRelocationType::X86_64_None,
                1 => ElfRelocationType::X86_64_64,
                2 => ElfRelocationType::X86_64_PC32,
                3 => ElfRelocationType::X86_64_GOT32,
                4 => ElfRelocationType::X86_64_PLT32,
                5 => ElfRelocationType::X86_64_Copy,
                6 => ElfRelocationType::X86_64_GlobDat,
                7 => ElfRelocationType::X86_64_JumpSlot,
                8 => ElfRelocationType::X86_64_Relative,
                9 => ElfRelocationType::X86_64_GOTPCRel,
                10 => ElfRelocationType::X86_64_32,
                11 => ElfRelocationType::X86_64_32S,
                12 => ElfRelocationType::X86_64_16,
                13 => ElfRelocationType::X86_64_PC16,
                14 => ElfRelocationType::X86_64_8,
                15 => ElfRelocationType::X86_64_PC8,
                16 => ElfRelocationType::X86_64_DTPMod64,
                17 => ElfRelocationType::X86_64_DTPOff64,
                18 => ElfRelocationType::X86_64_TPOff64,
                19 => ElfRelocationType::X86_64_TLSGD,
                20 => ElfRelocationType::X86_64_TLSLD,
                21 => ElfRelocationType::X86_64_DTPOff32,
                22 => ElfRelocationType::X86_64_GOTTPOff,
                23 => ElfRelocationType::X86_64_TPOff32,
                24 => ElfRelocationType::X86_64_PC64,
                25 => ElfRelocationType::X86_64_GOTOff64,
                26 => ElfRelocationType::X86_64_GOTPC32,
                32 => ElfRelocationType::X86_64_Size32,
                33 => ElfRelocationType::X86_64_Size64,
                34 => ElfRelocationType::X86_64_GOTPC32_TLSDesc,
                35 => ElfRelocationType::X86_64_TLSDescCall,
                36 => ElfRelocationType::X86_64_TLSDesc,
                37 => ElfRelocationType::X86_64_IRelative,
                other => ElfRelocationType::Unknown(other as _),
            },
        ),
        None => panic!("must call after the elf class is determined"),
    };
    let addend = if rela {
        Some(cursor.read_isize()?)
    } else {
        None
    };

    Ok(ElfRelocation {
        offset,
        symbol: PendingSymbolId(symbol_table, symbol),
        relocation_type,
        addend,
    })
}
