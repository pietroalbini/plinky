mod cursor;
mod dynamic;
mod header;
mod program_header;
mod sections;

pub use self::cursor::ReadSeek;
pub use self::dynamic::{ElfDynamicReader, ReadDynamicError};
use crate::errors::LoadError;
use crate::ids::ElfSectionId;
use crate::raw::RawSectionHeader;
use crate::reader::cursor::ReadCursor;
use crate::reader::header::read_header;
use crate::reader::sections::read_section;
use crate::{ElfClass, ElfEndian, ElfEnvironment, ElfObject, ElfSegment, ElfType};
use std::collections::BTreeMap;
use std::num::NonZeroU64;

pub struct ElfReader<'src> {
    cursor: ReadCursor<'src>,

    env: ElfEnvironment,
    type_: ElfType,
    entry: Option<NonZeroU64>,
    segments: Vec<ElfSegment>,
    sections: BTreeMap<ElfSectionId, RawSectionHeader>,
    section_names_table: ElfSectionId,
}

impl<'src> ElfReader<'src> {
    pub fn new<'a>(reader: &'a mut dyn ReadSeek) -> Result<ElfReader<'a>, LoadError> {
        // Default to elf32 LE for the header, it will be switched automatically.
        let cursor = ReadCursor::new(reader, ElfClass::Elf32, ElfEndian::Little);
        Self::new_inner(cursor)
    }

    pub fn new_owned(reader: Box<dyn ReadSeek>) -> Result<ElfReader<'static>, LoadError> {
        // Default to elf32 LE for the header, it will be switched automatically.
        let cursor = ReadCursor::new_owned(reader, ElfClass::Elf32, ElfEndian::Little);
        Self::new_inner(cursor)
    }

    fn new_inner<'a>(mut cursor: ReadCursor<'a>) -> Result<ElfReader<'a>, LoadError> {
        let header = read_header(&mut cursor)?;
        Ok(ElfReader {
            cursor,

            env: header.env,
            type_: header.type_,
            entry: header.entry,
            segments: header.segments,
            section_names_table: header.section_names_table,
            sections: header.sections,
        })
    }

    pub fn env(&self) -> ElfEnvironment {
        self.env
    }

    pub fn type_(&self) -> ElfType {
        self.type_
    }

    pub fn dynamic<'a>(&'a mut self) -> Result<ElfDynamicReader<'a, 'src>, ReadDynamicError> {
        ElfDynamicReader::new(self)
    }

    pub fn into_object(mut self) -> Result<ElfObject, LoadError> {
        let mut sections = BTreeMap::new();
        for (id, raw) in self.sections.into_iter() {
            sections.insert(id, read_section(&mut self.cursor, self.section_names_table, id, raw)?);
        }
        Ok(ElfObject {
            env: self.env,
            type_: self.type_,
            entry: self.entry,
            segments: self.segments,
            sections,
        })
    }
}
