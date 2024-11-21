mod cursor;
mod header;
mod program_header;
mod sections;

pub use self::cursor::ReadSeek;
use crate::errors::LoadError;
use crate::ids::ElfSectionId;
use crate::raw::RawSectionHeader;
use crate::reader::cursor::ReadCursor;
use crate::reader::header::read_header;
use crate::reader::sections::read_section;
use crate::{ElfClass, ElfEndian, ElfEnvironment, ElfObject, ElfSection, ElfSegment, ElfType};
use std::collections::BTreeMap;
use std::num::NonZeroU64;

pub struct ElfReader<'src> {
    cursor: ReadCursor<'src>,

    env: ElfEnvironment,
    type_: ElfType,
    entry: Option<NonZeroU64>,
    segments: Vec<ElfSegment>,
    sections: BTreeMap<ElfSectionId, MaybeSection>,
    section_names_table: ElfSectionId,
}

impl<'src> ElfReader<'src> {
    pub fn new(reader: &'src mut dyn ReadSeek) -> Result<Self, LoadError> {
        // Default to elf32 LE for the header, it will be switched automatically.
        let mut cursor = ReadCursor::new(reader, ElfClass::Elf32, ElfEndian::Little);
        let header = read_header(&mut cursor)?;

        Ok(ElfReader {
            cursor,
            env: header.env,
            type_: header.type_,
            entry: header.entry,
            segments: header.segments,
            sections: header
                .sections
                .into_iter()
                .map(|(id, raw)| (id, MaybeSection::Pending(raw)))
                .collect(),
            section_names_table: header.section_names_table,
        })
    }

    pub fn into_object(mut self) -> Result<ElfObject, LoadError> {
        let mut sections = BTreeMap::new();
        for (id, section) in self.sections.into_iter() {
            match section {
                MaybeSection::Pending(raw) => {
                    sections.insert(
                        id,
                        read_section(&mut self.cursor, self.section_names_table, id, raw)?,
                    );
                }
                MaybeSection::Ready(ready) => {
                    sections.insert(id, ready);
                }
            }
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

enum MaybeSection {
    Pending(RawSectionHeader),
    #[expect(unused)]
    Ready(ElfSection),
}
