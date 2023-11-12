use crate::errors::WriteError;
use crate::ids::ElfIds;
use crate::utils::WriteSeek;
use crate::{ElfClass, ElfObject};

pub(crate) struct WriteCursor<'a> {
    writer: &'a mut dyn WriteSeek,
    pub(crate) class: ElfClass,
}

impl<'a> WriteCursor<'a> {
    pub(super) fn new<I: ElfIds>(writer: &'a mut dyn WriteSeek, object: &ElfObject<I>) -> Self {
        Self {
            writer,
            class: object.env.class,
        }
    }

    pub(crate) fn write_usize(&mut self, value: u64) -> Result<(), WriteError> {
        match self.class {
            ElfClass::Elf32 => self.write_bytes(&(value as u32).to_le_bytes()),
            ElfClass::Elf64 => self.write_bytes(&value.to_le_bytes()),
        }
    }

    pub(crate) fn write_isize(&mut self, value: i64) -> Result<(), WriteError> {
        match self.class {
            ElfClass::Elf32 => self.write_bytes(&(value as i32).to_le_bytes()),
            ElfClass::Elf64 => self.write_bytes(&value.to_le_bytes()),
        }
    }

    pub(crate) fn write_bytes(&mut self, bytes: &[u8]) -> Result<(), WriteError> {
        self.writer.write_all(bytes)?;
        Ok(())
    }
}
