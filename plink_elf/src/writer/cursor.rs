use crate::errors::WriteError;
use crate::ids::ElfIds;
use crate::utils::WriteSeek;
use crate::{ElfClass, ElfEndian, ElfObject};
use std::io::SeekFrom;

pub(super) struct WriteCursor<'a> {
    writer: &'a mut dyn WriteSeek,
    class: ElfClass,
    endian: ElfEndian,
}

impl<'a> WriteCursor<'a> {
    pub(super) fn new<I: ElfIds>(writer: &'a mut dyn WriteSeek, object: &ElfObject<I>) -> Self {
        Self {
            writer,
            class: object.env.class,
            endian: object.env.endian,
        }
    }

    pub(super) fn write_u8(&mut self, value: u8) -> Result<(), WriteError> {
        self.writer.write_all(&[value])?;
        Ok(())
    }

    pub(super) fn write_u16(&mut self, value: u16) -> Result<(), WriteError> {
        self.writer.write_all(&match self.endian {
            ElfEndian::Little => u16::to_le_bytes(value),
            ElfEndian::Big => u16::to_be_bytes(value),
        })?;
        Ok(())
    }

    pub(super) fn write_u32(&mut self, value: u32) -> Result<(), WriteError> {
        self.writer.write_all(&match self.endian {
            ElfEndian::Little => u32::to_le_bytes(value),
            ElfEndian::Big => u32::to_be_bytes(value),
        })?;
        Ok(())
    }

    pub(super) fn write_u64(&mut self, value: u64) -> Result<(), WriteError> {
        self.writer.write_all(&match self.endian {
            ElfEndian::Little => u64::to_le_bytes(value),
            ElfEndian::Big => u64::to_be_bytes(value),
        })?;
        Ok(())
    }

    pub(super) fn write_usize(&mut self, value: u64) -> Result<(), WriteError> {
        match self.class {
            ElfClass::Elf32 => self.write_u32(value as u32),
            ElfClass::Elf64 => self.write_u64(value),
        }
    }

    pub(super) fn current_location(&mut self) -> Result<u64, WriteError> {
        Ok(self.writer.seek(SeekFrom::Current(0))?)
    }
}
