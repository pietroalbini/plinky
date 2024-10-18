mod dynamic;
mod group;
mod hash;
mod notes;
mod program;
mod relocations_table;
mod string_table;
mod symbol_table;
mod uninit;
mod unknown;

use crate::errors::LoadError;
use crate::ids::{ElfSectionId, ElfStringId};
use crate::raw::RawSectionHeader;
use crate::reader::ReadCursor;
use crate::{ElfDeduplication, ElfPermissions, ElfSection, ElfSectionContent};
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

    let mut reader = SectionReader { header: &header, cursor, section_id: current_section };

    // Ensure the deduplication flags are only applied to program sections.
    match (ty, reader.deduplication_flag()) {
        (SectionType::Program, _) => {}
        (_, Err(_) | Ok(ElfDeduplication::Disabled)) => {}
        _ => {
            return Err(LoadError::MergeFlagOnUnsupportedSection {
                section_idx: current_section.index,
            });
        }
    }

    let content = match ty {
        SectionType::Null => ElfSectionContent::Null,
        SectionType::Program => program::read(&mut reader)?,
        SectionType::SymbolTable { dynsym } => symbol_table::read(&mut reader, dynsym)?,
        SectionType::StringTable => string_table::read(&mut reader)?,
        SectionType::Relocations { rela } => relocations_table::read(&mut reader, rela)?,
        SectionType::Note => notes::read(&mut reader)?,
        SectionType::Uninit => uninit::read(&mut reader)?,
        SectionType::Group => group::read(&mut reader)?,
        SectionType::Hash => hash::read(&mut reader)?,
        SectionType::Dynamic => dynamic::read(&mut reader)?,
        SectionType::Unknown(other) => unknown::read(&mut reader, other)?,
    };

    Ok(ElfSection {
        name: ElfStringId { section: section_names_table, offset: header.name_offset },
        memory_address: header.memory_address,
        part_of_group: header.flags.group,
        content,
    })
}

#[derive(Clone, Copy)]
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

struct SectionReader<'a, 'b> {
    header: &'a RawSectionHeader,
    cursor: &'a mut ReadCursor<'b>,
    section_id: ElfSectionId,
}

impl SectionReader<'_, '_> {
    fn content(&mut self) -> Result<Vec<u8>, LoadError> {
        self.cursor.seek_to(self.header.offset)?;
        self.cursor.read_vec(self.header.size)
    }

    fn content_cursor(&mut self) -> Result<ReadCursor<'static>, LoadError> {
        let content = self.content()?;
        Ok(self.cursor_for(content))
    }

    fn cursor_for(&self, data: Vec<u8>) -> ReadCursor<'static> {
        let reader = std::io::Cursor::new(data);
        ReadCursor::new_owned(Box::new(reader), self.cursor.class, self.cursor.endian)
    }

    fn content_len(&self) -> u64 {
        self.header.size
    }

    fn section_link(&self) -> ElfSectionId {
        ElfSectionId { index: self.header.link }
    }

    fn section_info(&self) -> ElfSectionId {
        ElfSectionId { index: self.header.info }
    }

    fn permissions(&self) -> ElfPermissions {
        ElfPermissions {
            read: self.header.flags.alloc,
            write: self.header.flags.write,
            execute: self.header.flags.exec,
        }
    }

    fn deduplication_flag(&self) -> Result<ElfDeduplication, LoadError> {
        if self.header.flags.merge && self.header.flags.strings {
            Ok(ElfDeduplication::ZeroTerminatedStrings)
        } else if self.header.flags.merge {
            match NonZeroU64::new(self.header.entries_size) {
                None => {
                    return Err(LoadError::FixedSizeChunksMergeWithZeroLenChunks {
                        section_idx: self.section_id.index,
                    });
                }
                Some(size) => Ok(ElfDeduplication::FixedSizeChunks { size }),
            }
        } else {
            Ok(ElfDeduplication::Disabled)
        }
    }
}
