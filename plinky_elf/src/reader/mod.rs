mod cursor;
mod object;
mod program_header;
mod sections;

pub use self::cursor::ReadSeek;
use crate::errors::LoadError;
use crate::reader::cursor::ReadCursor;
use crate::reader::object::read_object;
use crate::{ElfClass, ElfEndian, ElfObject};

pub struct ElfReader<'src> {
    cursor: ReadCursor<'src>,
}

impl<'src> ElfReader<'src> {
    pub fn new(reader: &'src mut dyn ReadSeek) -> Result<Self, LoadError> {
        // Default to elf32 LE for the header, it will be switched automatically.
        let cursor = ReadCursor::new(reader, ElfClass::Elf32, ElfEndian::Little);

        Ok(ElfReader { cursor })
    }

    pub fn into_object(mut self) -> Result<ElfObject, LoadError> {
        read_object(&mut self.cursor)
    }
}
