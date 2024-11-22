use crate::ids::ElfSectionId;
use crate::raw::RawSectionHeader;
use crate::reader::cursor::ReadCursor;
use crate::{ElfDeduplication, ElfPermissions, LoadError};
use std::num::NonZeroU64;

pub(crate) struct SectionReader<'a, 'b> {
    pub(crate) parent_cursor: &'a mut ReadCursor<'b>,
    pub(crate) content_len: u64,
    pub(crate) content_start: u64,
}

impl SectionReader<'_, '_> {
    pub(super) fn content(&mut self) -> Result<Vec<u8>, LoadError> {
        self.parent_cursor.seek_to(self.content_start)?;
        self.parent_cursor.read_vec(self.content_len)
    }

    pub(super) fn content_cursor(&mut self) -> Result<ReadCursor<'static>, LoadError> {
        let content = self.content()?;
        Ok(self.cursor_for(content))
    }

    pub(super) fn cursor_for(&self, data: Vec<u8>) -> ReadCursor<'static> {
        let reader = std::io::Cursor::new(data);
        ReadCursor::new_owned(Box::new(reader), self.parent_cursor.class, self.parent_cursor.endian)
    }
}

pub(crate) trait SectionMetadata {
    fn info_field(&self) -> u32;
    fn section_id(&self) -> ElfSectionId;
    fn section_link(&self) -> ElfSectionId;
    fn section_info(&self) -> ElfSectionId;
    fn permissions(&self) -> ElfPermissions;
    fn deduplication_flag(&self) -> Result<ElfDeduplication, LoadError>;
}

pub(super) struct HeaderMetadata<'a> {
    header: &'a RawSectionHeader,
    section_id: ElfSectionId,
}

impl<'a> HeaderMetadata<'a> {
    pub(super) fn new(header: &'a RawSectionHeader, section_id: ElfSectionId) -> Self {
        Self { header, section_id }
    }
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
