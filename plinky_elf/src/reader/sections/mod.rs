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
use std::num::NonZeroU64;

pub(super) fn read_section(
    cursor: &mut ReadCursor<'_>,
    section_names_table: ElfSectionId,
    current_section: ElfSectionId,
    header: RawSectionHeader,
) -> Result<ElfSection, LoadError> {
    read_section_inner(cursor, section_names_table, current_section, header).map_err(|inner| {
        LoadError::FailedToParseSection { idx: current_section.index as _, inner: Box::new(inner) }
    })
}

fn read_section_inner(
    cursor: &mut ReadCursor<'_>,
    section_names_table: ElfSectionId,
    current_section: ElfSectionId,
    header: RawSectionHeader,
) -> Result<ElfSection, LoadError> {
    let ty = match header.type_ {
        0 => SectionType::Null,
        1 => SectionType::Program,
        2 => SectionType::SymbolTable { dynsym: false },
        3 => SectionType::StringTable,
        4 => SectionType::Rela,
        5 => SectionType::Hash,
        6 => SectionType::Dynamic,
        7 => SectionType::Note,
        8 => SectionType::Uninit,
        9 => SectionType::Rel,
        11 => SectionType::SymbolTable { dynsym: true },
        17 => SectionType::Group,
        other => SectionType::Unknown(other),
    };

    // The info link flag is used to indicate the info field contains a link to a section table,
    // which only makes sense for relocations. The flag doesn't actually seem to be required
    // though, as for example GCC emits it while NASM doesn't. To catch unknown uses of the flag,
    // we error out if the flag is set for a non-relocation section.
    if header.flags.info_link && !matches!(ty, SectionType::Rel | SectionType::Rela) {
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

    let mut reader = SectionReader {
        parent_cursor: cursor,
        content_len: header.size,
        content_start: header.offset,
    };
    let meta = HeaderMetadata { header: &header, section_id: current_section };

    // Ensure the deduplication flags are only applied to program sections.
    match (ty, meta.deduplication_flag()) {
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
        SectionType::Program => program::read(&mut reader, &meta)?,
        SectionType::SymbolTable { dynsym } => symbol_table::read(&mut reader, &meta, dynsym)?,
        SectionType::StringTable => string_table::read(&mut reader)?,
        SectionType::Rel => relocations_table::read_rel(&mut reader, &meta)?,
        SectionType::Rela => relocations_table::read_rela(&mut reader, &meta)?,
        SectionType::Note => notes::read(&mut reader)?,
        SectionType::Uninit => uninit::read(&mut reader, &meta)?,
        SectionType::Group => group::read(&mut reader, &meta)?,
        SectionType::Hash => hash::read(&mut reader, &meta)?,
        SectionType::Dynamic => dynamic::read(&mut reader, &meta)?,
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
    Rel,
    Rela,
    Note,
    Uninit,
    Group,
    Hash,
    Dynamic,
    Unknown(u32),
}

struct SectionReader<'a, 'b> {
    parent_cursor: &'a mut ReadCursor<'b>,
    content_len: u64,
    content_start: u64,
}

impl SectionReader<'_, '_> {
    fn content(&mut self) -> Result<Vec<u8>, LoadError> {
        self.parent_cursor.seek_to(self.content_start)?;
        self.parent_cursor.read_vec(self.content_len)
    }

    fn content_cursor(&mut self) -> Result<ReadCursor<'static>, LoadError> {
        let content = self.content()?;
        Ok(self.cursor_for(content))
    }

    fn cursor_for(&self, data: Vec<u8>) -> ReadCursor<'static> {
        let reader = std::io::Cursor::new(data);
        ReadCursor::new_owned(Box::new(reader), self.parent_cursor.class, self.parent_cursor.endian)
    }
}

trait SectionMetadata {
    fn info_field(&self) -> u32;
    fn section_id(&self) -> ElfSectionId;
    fn section_link(&self) -> ElfSectionId;
    fn section_info(&self) -> ElfSectionId;
    fn permissions(&self) -> ElfPermissions;
    fn deduplication_flag(&self) -> Result<ElfDeduplication, LoadError>;
}

struct HeaderMetadata<'a> {
    header: &'a RawSectionHeader,
    section_id: ElfSectionId,
}

impl SectionMetadata for HeaderMetadata<'_> {
    fn info_field(&self) -> u32 {
        self.header.info
    }

    fn section_id(&self) -> ElfSectionId {
        self.section_id
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
