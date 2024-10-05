mod dynamic;
mod group;
mod hash;
mod notes;
mod relocations_table;
mod string_table;
mod symbol_table;

use crate::errors::LoadError;
use crate::ids::{ElfSectionId, ElfStringId};
use crate::raw::RawSectionHeader;
use crate::reader::ReadCursor;
use crate::{
    ElfDeduplication, ElfPermissions, ElfProgramSection, ElfSection, ElfSectionContent,
    ElfUninitializedSection, ElfUnknownSection,
};
use std::collections::BTreeMap;
use std::num::NonZeroU64;

pub(super) fn read_sections(
    cursor: &mut ReadCursor<'_>,
    offset: u64,
    count: u16,
    size: u16,
    section_names_table: ElfSectionId,
) -> Result<BTreeMap<ElfSectionId, ElfSection>, LoadError> {
    if offset == 0 {
        return Ok(BTreeMap::new());
    }

    let mut sections = BTreeMap::new();
    for idx in 0..count {
        cursor.seek_to(offset + (size as u64 * idx as u64))?;
        sections.insert(
            ElfSectionId { index: idx.into() },
            read_section(cursor, section_names_table, ElfSectionId { index: idx.into() })
                .map_err(|inner| LoadError::FailedToParseSection { idx, inner: Box::new(inner) })?,
        );
    }

    Ok(sections)
}

fn read_section(
    cursor: &mut ReadCursor<'_>,
    section_names_table: ElfSectionId,
    current_section: ElfSectionId,
) -> Result<ElfSection, LoadError> {
    let header: RawSectionHeader = cursor.read_raw().map_err(|e| {
        LoadError::FailedToParseSectionHeader { idx: current_section.index, inner: Box::new(e) }
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
        return Err(LoadError::UnsupportedInfoLinkFlag(current_section.index));
    }

    if header.flags.strings {
        // The spec says the entries_size field determines how long each char is, but there is no
        // point implementing support for this unless an actual object needs it. Error out for now
        // if this happens, to avoid malformed programs being emitted.
        if header.entries_size != 1 {
            return Err(LoadError::UnsupportedStringsWithSizeNotOne {
                section_idx: current_section.index,
                size: header.entries_size,
            });
        }
        // Not sure if there is any valid use of SHF_STRINGS outside of SHF_MERGE or it being
        // redundantly applied to string tables. Error out for now, if a valid use is found the
        // linker will need to be updated to handle it.
        if !(header.flags.merge || matches!(ty, SectionType::StringTable)) {
            return Err(LoadError::UnexpectedStringsFlag { section_idx: current_section.index });
        }
    }

    let mut deduplication = if header.flags.merge && header.flags.strings {
        Some(ElfDeduplication::ZeroTerminatedStrings)
    } else if header.flags.merge {
        match NonZeroU64::new(header.entries_size) {
            None => {
                return Err(LoadError::FixedSizeChunksMergeWithZeroLenChunks {
                    section_idx: current_section.index,
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
            symbol_table::read(
                cursor,
                &raw,
                ElfSectionId { index: header.link },
                current_section,
                dynsym,
            )?
        }
        SectionType::StringTable => {
            string_table::read(&read_section_raw_content(&header, cursor)?)?
        }
        SectionType::Relocations { rela } => {
            let raw = read_section_raw_content(&header, cursor)?;
            relocations_table::read(
                cursor,
                &raw,
                ElfSectionId { index: header.link },
                ElfSectionId { index: header.info },
                rela,
            )?
        }
        SectionType::Note => {
            let raw = read_section_raw_content(&header, cursor)?;
            notes::read(cursor, &raw)?
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
            group::read(&header, cursor, &raw)?
        }
        SectionType::Hash => {
            let raw = read_section_raw_content(&header, cursor)?;
            hash::read(&header, &raw, cursor)?
        }
        SectionType::Dynamic => {
            let raw = read_section_raw_content(&header, cursor)?;
            dynamic::read(&header, &raw, cursor)?
        }
        SectionType::Unknown(other) => ElfSectionContent::Unknown(ElfUnknownSection {
            id: other,
            raw: read_section_raw_content(&header, cursor)?,
        }),
    };

    if deduplication.is_some() {
        return Err(LoadError::MergeFlagOnUnsupportedSection {
            section_idx: current_section.index,
        });
    }

    Ok(ElfSection {
        name: ElfStringId { section: section_names_table, offset: header.name_offset },
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
