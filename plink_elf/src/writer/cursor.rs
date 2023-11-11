use crate::errors::WriteError;
use crate::ids::ElfIds;
use crate::utils::WriteSeek;
use crate::{ElfClass, ElfObject};
use std::io::SeekFrom;

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

    pub(super) fn write_u16(&mut self, value: u16) -> Result<(), WriteError> {
        self.writer.write_all(&value.to_le_bytes())?;
        Ok(())
    }

    pub(super) fn write_u32(&mut self, value: u32) -> Result<(), WriteError> {
        self.writer.write_all(&value.to_le_bytes())?;
        Ok(())
    }

    pub(super) fn write_u64(&mut self, value: u64) -> Result<(), WriteError> {
        self.writer.write_all(&value.to_le_bytes())?;
        Ok(())
    }

    pub(super) fn write_i32(&mut self, value: i32) -> Result<(), WriteError> {
        self.writer.write_all(&value.to_le_bytes())?;
        Ok(())
    }

    pub(super) fn write_i64(&mut self, value: i64) -> Result<(), WriteError> {
        self.writer.write_all(&value.to_le_bytes())?;
        Ok(())
    }

    pub(crate) fn write_usize(&mut self, value: u64) -> Result<(), WriteError> {
        match self.class {
            ElfClass::Elf32 => self.write_u32(value as u32),
            ElfClass::Elf64 => self.write_u64(value),
        }
    }

    pub(crate) fn write_isize(&mut self, value: i64) -> Result<(), WriteError> {
        match self.class {
            ElfClass::Elf32 => self.write_i32(value as i32),
            ElfClass::Elf64 => self.write_i64(value),
        }
    }

    pub(crate) fn write_bytes(&mut self, bytes: &[u8]) -> Result<(), WriteError> {
        self.writer.write_all(bytes)?;
        Ok(())
    }

    pub(super) fn current_location(&mut self) -> Result<u64, WriteError> {
        Ok(self.writer.seek(SeekFrom::Current(0))?)
    }
}
