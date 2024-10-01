use super::{PendingStringId, PendingSymbolId};
use crate::errors::LoadError;
use crate::raw::{RawGroupFlags, RawHashHeader, RawRel, RawRela, RawSectionHeader, RawSymbol};
use crate::reader::notes::read_notes;
use crate::reader::{PendingIds, PendingSectionId, ReadCursor};
use crate::{
    ElfClass, ElfDeduplication, ElfDynamic, ElfDynamicDirective, ElfDynamicFlags, ElfDynamicFlags1,
    ElfGroup, ElfHash, ElfPLTRelocationsMode, ElfPermissions, ElfProgramSection, ElfRelocation,
    ElfRelocationType, ElfRelocationsTable, ElfSection, ElfSectionContent, ElfStringTable,
    ElfSymbol, ElfSymbolBinding, ElfSymbolDefinition, ElfSymbolTable, ElfSymbolType,
    ElfSymbolVisibility, ElfUninitializedSection, ElfUnknownSection,
};
use plinky_utils::bitfields::Bitfield;
use std::collections::BTreeMap;
use std::num::NonZeroU64;

pub(super) fn read_sections(
    cursor: &mut ReadCursor<'_>,
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
            read_section(cursor, section_names_table, PendingSectionId(idx as _))
                .map_err(|inner| LoadError::FailedToParseSection { idx, inner: Box::new(inner) })?,
        );
    }

    Ok(sections)
}

fn read_section(
    cursor: &mut ReadCursor<'_>,
    section_names_table: PendingSectionId,
    current_section: PendingSectionId,
) -> Result<ElfSection<PendingIds>, LoadError> {
    let header: RawSectionHeader = cursor.read_raw().map_err(|e| {
        LoadError::FailedToParseSectionHeader { idx: current_section.0, inner: Box::new(e) }
    })?;

    let ty = match header.type_ {
        0 => SectionType::Null,
        1 => SectionType::Program,
        2 => SectionType::SymbolTable { dynsym: false },
        3 => SectionType::StringTable,
        4 => SectionType::Relocations { rela: true },
        5 => SectionType::Hash,
        6 => SectionType::Dynamic,
        7 => SectionType::Note,
        8 => SectionType::Uninit,
        9 => SectionType::Relocations { rela: false },
        11 => SectionType::SymbolTable { dynsym: true },
        17 => SectionType::Group,
        other => SectionType::Unknown(other),
    };

    // The info link flag is used to indicate the info field contains a link to a section table,
    // which only makes sense for relocations. The flag doesn't actually seem to be required
    // though, as for example GCC emits it while NASM doesn't. To catch unknown uses of the flag,
    // we error out if the flag is set for a non-relocation section.
    if header.flags.info_link && !matches!(ty, SectionType::Relocations { .. }) {
        return Err(LoadError::UnsupportedInfoLinkFlag(current_section.0));
    }

    if header.flags.strings {
        // The spec says the entries_size field determines how long each char is, but there is no
        // point implementing support for this unless an actual object needs it. Error out for now
        // if this happens, to avoid malformed programs being emitted.
        if header.entries_size != 1 {
            return Err(LoadError::UnsupportedStringsWithSizeNotOne {
                section_idx: current_section.0,
                size: header.entries_size,
            });
        }
        // Not sure if there is any valid use of SHF_STRINGS outside of SHF_MERGE or it being
        // redundantly applied to string tables. Error out for now, if a valid use is found the
        // linker will need to be updated to handle it.
        if !(header.flags.merge || matches!(ty, SectionType::StringTable)) {
            return Err(LoadError::UnexpectedStringsFlag { section_idx: current_section.0 });
        }
    }

    let mut deduplication = if header.flags.merge && header.flags.strings {
        Some(ElfDeduplication::ZeroTerminatedStrings)
    } else if header.flags.merge {
        match NonZeroU64::new(header.entries_size) {
            None => {
                return Err(LoadError::FixedSizeChunksMergeWithZeroLenChunks {
                    section_idx: current_section.0,
                })
            }
            Some(size) => Some(ElfDeduplication::FixedSizeChunks { size }),
        }
    } else {
        None
    };

    let content = match ty {
        SectionType::Null => ElfSectionContent::Null,
        SectionType::Program => ElfSectionContent::Program(ElfProgramSection {
            perms: ElfPermissions {
                read: header.flags.alloc,
                write: header.flags.write,
                execute: header.flags.exec,
            },
            deduplication: deduplication.take().unwrap_or(ElfDeduplication::Disabled),
            raw: read_section_raw_content(&header, cursor)?,
        }),
        SectionType::SymbolTable { dynsym } => {
            let raw = read_section_raw_content(&header, cursor)?;
            read_symbol_table(cursor, &raw, PendingSectionId(header.link), current_section, dynsym)?
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
        SectionType::Group => {
            let raw = read_section_raw_content(&header, cursor)?;
            ElfSectionContent::Group(read_group(&header, cursor, &raw)?)
        }
        SectionType::Hash => {
            let raw = read_section_raw_content(&header, cursor)?;
            ElfSectionContent::Hash(read_hash(&header, &raw, cursor)?)
        }
        SectionType::Dynamic => {
            let raw = read_section_raw_content(&header, cursor)?;
            ElfSectionContent::Dynamic(read_dynamic(&header, &raw, cursor)?)
        }
        SectionType::Unknown(other) => ElfSectionContent::Unknown(ElfUnknownSection {
            id: other,
            raw: read_section_raw_content(&header, cursor)?,
        }),
    };

    if deduplication.is_some() {
        return Err(LoadError::MergeFlagOnUnsupportedSection { section_idx: current_section.0 });
    }

    Ok(ElfSection {
        name: PendingStringId(section_names_table, header.name_offset),
        memory_address: header.memory_address,
        part_of_group: header.flags.group,
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
    SymbolTable { dynsym: bool },
    StringTable,
    Relocations { rela: bool },
    Note,
    Uninit,
    Group,
    Hash,
    Dynamic,
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
    dynsym: bool,
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

    Ok(ElfSectionContent::SymbolTable(ElfSymbolTable { dynsym, symbols }))
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
                3 => ElfRelocationType::X86_GOT32,
                4 => ElfRelocationType::X86_PLT32,
                5 => ElfRelocationType::X86_COPY,
                6 => ElfRelocationType::X86_GlobDat,
                7 => ElfRelocationType::X86_JumpSlot,
                8 => ElfRelocationType::X86_Relative,
                9 => ElfRelocationType::X86_GOTOff,
                10 => ElfRelocationType::X86_GOTPC,
                11 => ElfRelocationType::X86_GOT32X,
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
                38 => ElfRelocationType::X86_64_IRelative64,
                41 => ElfRelocationType::X86_64_GOTPCRelX,
                42 => ElfRelocationType::X86_64_Rex_GOTPCRelX,
                43 => ElfRelocationType::X86_64_Code_4_GOTPCRelX,
                44 => ElfRelocationType::X86_64_Code_4_GOTPCOff,
                45 => ElfRelocationType::X86_64_Code_4_GOTPC32_TLSDesc,
                46 => ElfRelocationType::X86_64_Code_5_GOTPCRelX,
                47 => ElfRelocationType::X86_64_Code_5_GOTPCOff,
                48 => ElfRelocationType::X86_64_Code_5_GOTPC32_TLSDesc,
                49 => ElfRelocationType::X86_64_Code_6_GOTPCRelX,
                50 => ElfRelocationType::X86_64_Code_6_GOTPCOff,
                51 => ElfRelocationType::X86_64_Code_6_GOTPC32_TLSDesc,
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

fn read_group(
    header: &RawSectionHeader,
    cursor: &mut ReadCursor<'_>,
    raw_content: &[u8],
) -> Result<ElfGroup<PendingIds>, LoadError> {
    let mut inner = std::io::Cursor::new(raw_content);
    let mut cursor = cursor.duplicate(&mut inner);

    let symbol_table = PendingSectionId(header.link);
    let signature = PendingSymbolId(symbol_table, header.info);

    let flags: RawGroupFlags = cursor.read_raw()?;

    let mut sections = Vec::new();
    while cursor.current_position()? < raw_content.len() as u64 {
        sections.push(PendingSectionId(cursor.read_raw::<u32>()?));
    }

    Ok(ElfGroup { symbol_table, signature, sections, comdat: flags.comdat })
}

fn read_hash(
    header: &RawSectionHeader,
    raw_content: &[u8],
    cursor: &mut ReadCursor,
) -> Result<ElfHash<PendingIds>, LoadError> {
    let mut inner = std::io::Cursor::new(raw_content);
    let mut cursor = cursor.duplicate(&mut inner);

    let hash_header: RawHashHeader = cursor.read_raw()?;
    let mut hash = ElfHash {
        symbol_table: PendingSectionId(header.link),
        buckets: Vec::with_capacity(hash_header.bucket_count as _),
        chain: Vec::with_capacity(hash_header.chain_count as _),
    };
    for _ in 0..hash_header.bucket_count {
        hash.buckets.push(cursor.read_raw()?);
    }
    for _ in 0..hash_header.chain_count {
        hash.chain.push(cursor.read_raw()?);
    }
    Ok(hash)
}

fn read_dynamic(
    header: &RawSectionHeader,
    raw_content: &[u8],
    cursor: &mut ReadCursor,
) -> Result<ElfDynamic<PendingIds>, LoadError> {
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

    Ok(ElfDynamic { string_table: PendingSectionId(header.link), directives })
}
