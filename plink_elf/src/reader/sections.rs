use super::{PendingStringId, PendingSymbolId};
use crate::errors::LoadError;
use crate::reader::notes::read_notes;
use crate::reader::{Cursor, PendingIds, PendingSectionId};
use crate::{
    Class, ProgramSection, RawBytes, Relocation, RelocationType, RelocationsTable, Section,
    SectionContent, StringTable, Symbol, SymbolBinding, SymbolDefinition, SymbolTable, SymbolType,
    UnknownSection,
};
use std::collections::BTreeMap;

pub(super) fn read_sections(
    cursor: &mut Cursor<'_>,
    offset: u64,
    count: u16,
    size: u16,
    section_names_table: PendingSectionId,
) -> Result<BTreeMap<PendingSectionId, Section<PendingIds>>, LoadError> {
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
) -> Result<Section<PendingIds>, LoadError> {
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
        0 => SectionContent::Null,
        1 => SectionContent::Program(ProgramSection {
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
        7 => SectionContent::Note(read_notes(cursor, &raw_content)?),
        9 => read_relocations_table(
            cursor,
            &raw_content,
            PendingSectionId(link),
            PendingSectionId(info),
            false,
        )?,
        other => SectionContent::Unknown(UnknownSection {
            id: other,
            raw: RawBytes(raw_content),
        }),
    };

    Ok(Section {
        name: PendingStringId(section_names_table, name_offset),
        memory_address,
        content,
    })
}

fn read_string_table(raw_content: &[u8]) -> Result<SectionContent<PendingIds>, LoadError> {
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
    Ok(SectionContent::StringTable(StringTable::new(strings)))
}

fn read_symbol_table(
    cursor: &mut Cursor<'_>,
    raw_content: &[u8],
    strings_table: PendingSectionId,
    current_section: PendingSectionId,
) -> Result<SectionContent<PendingIds>, LoadError> {
    let mut inner = std::io::Cursor::new(raw_content);
    let mut cursor = cursor.duplicate(&mut inner);

    let mut symbols = BTreeMap::new();
    while cursor.current_position()? != raw_content.len() as u64 {
        symbols.insert(
            PendingSymbolId(current_section, symbols.len() as _),
            read_symbol(&mut cursor, strings_table)?,
        );
    }

    Ok(SectionContent::SymbolTable(SymbolTable { symbols }))
}

fn read_symbol(
    cursor: &mut Cursor<'_>,
    strings_table: PendingSectionId,
) -> Result<Symbol<PendingIds>, LoadError> {
    let mut value = 0;
    let mut size = 0;

    let name_offset = cursor.read_u32()?;
    if let Some(Class::Elf32) = cursor.class {
        value = cursor.read_usize()?;
        size = cursor.read_usize()?;
    }
    let info = cursor.read_u8()?;
    let _ = cursor.read_u8()?; // Reserved
    let definition = cursor.read_u16()?;
    if let Some(Class::Elf64) = cursor.class {
        value = cursor.read_usize()?;
        size = cursor.read_usize()?;
    }

    Ok(Symbol {
        name: PendingStringId(strings_table, name_offset),
        binding: match (info & 0b11110000) >> 4 {
            0 => SymbolBinding::Local,
            1 => SymbolBinding::Global,
            2 => SymbolBinding::Weak,
            other => SymbolBinding::Unknown(other),
        },
        type_: match info & 0b1111 {
            0 => SymbolType::NoType,
            1 => SymbolType::Object,
            2 => SymbolType::Function,
            3 => SymbolType::Section,
            4 => SymbolType::File,
            other => SymbolType::Unknown(other),
        },
        definition: match definition {
            0x0000 => SymbolDefinition::Undefined, // SHN_UNDEF
            0xFFF1 => SymbolDefinition::Absolute,  // SHN_ABS
            0xFFF2 => SymbolDefinition::Common,    // SHN_COMMON
            other => SymbolDefinition::Section(PendingSectionId(other as _)),
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
) -> Result<SectionContent<PendingIds>, LoadError> {
    let mut inner = std::io::Cursor::new(raw_content);
    let mut cursor = cursor.duplicate(&mut inner);

    let mut relocations = Vec::new();
    while cursor.current_position()? != raw_content.len() as u64 {
        relocations.push(read_relocation(&mut cursor, symbol_table, rela)?);
    }

    Ok(SectionContent::RelocationsTable(RelocationsTable {
        symbol_table,
        applies_to_section,
        relocations,
    }))
}

fn read_relocation(
    cursor: &mut Cursor<'_>,
    symbol_table: PendingSectionId,
    rela: bool,
) -> Result<Relocation<PendingIds>, LoadError> {
    let offset = cursor.read_usize()?;
    let info = cursor.read_usize()?;
    let (symbol, relocation_type) = match cursor.class {
        Some(Class::Elf32) => (
            (info >> 8) as u32,
            match info & 0xF {
                0 => RelocationType::X86_None,
                1 => RelocationType::X86_32,
                2 => RelocationType::X86_PC32,
                other => RelocationType::Unknown(other as _),
            },
        ),
        Some(Class::Elf64) => (
            (info >> 32) as u32,
            match info & 0xFFFF_FFFF {
                0 => RelocationType::X86_64_None,
                1 => RelocationType::X86_64_64,
                2 => RelocationType::X86_64_PC32,
                3 => RelocationType::X86_64_GOT32,
                4 => RelocationType::X86_64_PLT32,
                5 => RelocationType::X86_64_Copy,
                6 => RelocationType::X86_64_GlobDat,
                7 => RelocationType::X86_64_JumpSlot,
                8 => RelocationType::X86_64_Relative,
                9 => RelocationType::X86_64_GOTPCRel,
                10 => RelocationType::X86_64_32,
                11 => RelocationType::X86_64_32S,
                12 => RelocationType::X86_64_16,
                13 => RelocationType::X86_64_PC16,
                14 => RelocationType::X86_64_8,
                15 => RelocationType::X86_64_PC8,
                16 => RelocationType::X86_64_DTPMod64,
                17 => RelocationType::X86_64_DTPOff64,
                18 => RelocationType::X86_64_TPOff64,
                19 => RelocationType::X86_64_TLSGD,
                20 => RelocationType::X86_64_TLSLD,
                21 => RelocationType::X86_64_DTPOff32,
                22 => RelocationType::X86_64_GOTTPOff,
                23 => RelocationType::X86_64_TPOff32,
                24 => RelocationType::X86_64_PC64,
                25 => RelocationType::X86_64_GOTOff64,
                26 => RelocationType::X86_64_GOTPC32,
                32 => RelocationType::X86_64_Size32,
                33 => RelocationType::X86_64_Size64,
                34 => RelocationType::X86_64_GOTPC32_TLSDesc,
                35 => RelocationType::X86_64_TLSDescCall,
                36 => RelocationType::X86_64_TLSDesc,
                37 => RelocationType::X86_64_IRelative,
                other => RelocationType::Unknown(other as _),
            },
        ),
        None => panic!("must call after the elf class is determined"),
    };
    let addend = if rela {
        Some(cursor.read_isize()?)
    } else {
        None
    };

    Ok(Relocation {
        offset,
        symbol: PendingSymbolId(symbol_table, symbol),
        relocation_type,
        addend,
    })
}
