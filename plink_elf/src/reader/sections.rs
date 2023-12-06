use super::{PendingStringId, PendingSymbolId};
use crate::errors::LoadError;
use crate::raw::{RawRel, RawRela, RawSectionHeader, RawSymbol};
use crate::reader::notes::read_notes;
use crate::reader::program_header::SegmentContentMapping;
use crate::reader::{PendingIds, PendingSectionId, ReadCursor};
use crate::{
    ElfClass, ElfPermissions, ElfProgramSection, ElfRelocation, ElfRelocationType,
    ElfRelocationsTable, ElfSection, ElfSectionContent, ElfSegmentContent, ElfStringTable,
    ElfSymbol, ElfSymbolBinding, ElfSymbolDefinition, ElfSymbolTable, ElfSymbolType,
    ElfUninitializedSection, ElfUnknownSection, RawBytes,
};
use std::collections::BTreeMap;

pub(super) fn read_sections(
    cursor: &mut ReadCursor<'_>,
    segment_content_map: &mut SegmentContentMapping,
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
            read_section(
                cursor,
                segment_content_map,
                section_names_table,
                PendingSectionId(idx as _),
            )?,
        );
    }

    Ok(sections)
}

fn read_section(
    cursor: &mut ReadCursor<'_>,
    segment_content_map: &mut SegmentContentMapping,
    section_names_table: PendingSectionId,
    current_section: PendingSectionId,
) -> Result<ElfSection<PendingIds>, LoadError> {
    let header: RawSectionHeader =
        cursor
            .read_raw()
            .map_err(|e| LoadError::FailedToParseSectionHeader {
                idx: current_section.0,
                inner: Box::new(e),
            })?;

    let ty = match header.type_ {
        0 => SectionType::Null,
        1 => SectionType::Program,
        2 => SectionType::SymbolTable,
        3 => SectionType::StringTable,
        4 => SectionType::Relocations { rela: true },
        7 => SectionType::Note,
        8 => SectionType::Uninit,
        9 => SectionType::Relocations { rela: false },
        other => SectionType::Unknown(other),
    };

    // The info link flag is used to indicate the info field contains a link to a section table,
    // which only makes sense for relocations. The flag doesn't actually seem to be required
    // though, as for example GCC emits it while NASM doesn't. To catch unknown uses of the flag,
    // we error out if the flag is set for a non-relocation section.
    if header.flags.info_link && !matches!(ty, SectionType::Relocations { .. }) {
        return Err(LoadError::UnsupportedInfoLinkFlag(current_section.0));
    }

    let content = match ty {
        SectionType::Null => ElfSectionContent::Null,
        SectionType::Program => ElfSectionContent::Program(ElfProgramSection {
            perms: ElfPermissions {
                read: header.flags.alloc,
                write: header.flags.write,
                execute: header.flags.exec,
            },
            raw: RawBytes(read_section_raw_content(&header, cursor)?),
        }),
        SectionType::SymbolTable => {
            let raw = read_section_raw_content(&header, cursor)?;
            read_symbol_table(cursor, &raw, PendingSectionId(header.link), current_section)?
        }
        SectionType::StringTable => read_string_table(&read_section_raw_content(&header, cursor)?)?,
        SectionType::Relocations { rela } => {
            let raw = read_section_raw_content(&header, cursor)?;
            read_relocations_table(
                cursor,
                &raw,
                PendingSectionId(header.link),
                PendingSectionId(header.info),
                rela,
            )?
        }
        SectionType::Note => {
            let raw = read_section_raw_content(&header, cursor)?;
            ElfSectionContent::Note(read_notes(cursor, &raw)?)
        }
        SectionType::Uninit => ElfSectionContent::Uninitialized(ElfUninitializedSection {
            perms: ElfPermissions {
                read: header.flags.alloc,
                write: header.flags.write,
                execute: header.flags.exec,
            },
            len: header.size,
        }),
        SectionType::Unknown(other) => ElfSectionContent::Unknown(ElfUnknownSection {
            id: other,
            raw: RawBytes(read_section_raw_content(&header, cursor)?),
        }),
    };

    segment_content_map.insert(
        (header.offset, header.size),
        ElfSegmentContent::Section(current_section),
    );

    Ok(ElfSection {
        name: PendingStringId(section_names_table, header.name_offset),
        memory_address: header.memory_address,
        content,
    })
}

fn read_section_raw_content(
    header: &RawSectionHeader,
    cursor: &mut ReadCursor<'_>,
) -> Result<Vec<u8>, LoadError> {
    cursor.seek_to(header.offset)?;
    cursor.read_vec(header.size)
}

enum SectionType {
    Null,
    Program,
    SymbolTable,
    StringTable,
    Relocations { rela: bool },
    Note,
    Uninit,
    Unknown(u32),
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
    cursor: &mut ReadCursor<'_>,
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
    cursor: &mut ReadCursor<'_>,
    strings_table: PendingSectionId,
) -> Result<ElfSymbol<PendingIds>, LoadError> {
    let symbol: RawSymbol = cursor.read_raw()?;
    Ok(ElfSymbol {
        name: PendingStringId(strings_table, symbol.name_offset),
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
        definition: match symbol.definition {
            0x0000 => ElfSymbolDefinition::Undefined, // SHN_UNDEF
            0xFFF1 => ElfSymbolDefinition::Absolute,  // SHN_ABS
            0xFFF2 => ElfSymbolDefinition::Common,    // SHN_COMMON
            other => ElfSymbolDefinition::Section(PendingSectionId(other as _)),
        },
        value: symbol.value,
        size: symbol.size,
    })
}

fn read_relocations_table(
    cursor: &mut ReadCursor<'_>,
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
    cursor: &mut ReadCursor<'_>,
    symbol_table: PendingSectionId,
    rela: bool,
) -> Result<ElfRelocation<PendingIds>, LoadError> {
    let (offset, info, addend) = if rela {
        let raw: RawRela = cursor.read_raw()?;
        (raw.offset, raw.info, Some(raw.addend))
    } else {
        let raw: RawRel = cursor.read_raw()?;
        (raw.offset, raw.info, None)
    };
    let (symbol, relocation_type) = match cursor.class {
        ElfClass::Elf32 => (
            (info >> 8) as u32,
            match info & 0xF {
                0 => ElfRelocationType::X86_None,
                1 => ElfRelocationType::X86_32,
                2 => ElfRelocationType::X86_PC32,
                other => ElfRelocationType::Unknown(other as _),
            },
        ),
        ElfClass::Elf64 => (
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
    };

    Ok(ElfRelocation {
        offset,
        symbol: PendingSymbolId(symbol_table, symbol),
        relocation_type,
        addend,
    })
}
