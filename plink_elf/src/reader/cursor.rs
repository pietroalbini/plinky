use crate::errors::LoadError;
use crate::utils::ReadSeek;
use crate::ElfClass;
use std::io::SeekFrom;

pub(crate) struct ReadCursor<'a> {
    reader: &'a mut dyn ReadSeek,
    pub(super) class: Option<ElfClass>,
}

impl<'a> ReadCursor<'a> {
    pub(crate) fn new(reader: &'a mut dyn ReadSeek) -> Self {
        Self {
            reader,
            class: None,
        }
    }

    pub(super) fn seek_to(&mut self, position: u64) -> Result<(), LoadError> {
        self.reader.seek(SeekFrom::Start(position))?;
        Ok(())
    }

    pub(super) fn read_u8(&mut self) -> Result<u8, LoadError> {
        let bytes = self.read_bytes::<1>()?;
        Ok(bytes[0])
    }

    pub(super) fn read_u16(&mut self) -> Result<u16, LoadError> {
        Ok(u16::from_le_bytes(self.read_bytes()?))
    }

    pub(super) fn read_u32(&mut self) -> Result<u32, LoadError> {
        Ok(u32::from_le_bytes(self.read_bytes()?))
    }

    pub(super) fn read_u64(&mut self) -> Result<u64, LoadError> {
        Ok(u64::from_le_bytes(self.read_bytes()?))
    }

    pub(super) fn read_i32(&mut self) -> Result<i32, LoadError> {
        Ok(i32::from_le_bytes(self.read_bytes()?))
    }

    pub(super) fn read_i64(&mut self) -> Result<i64, LoadError> {
        Ok(i64::from_le_bytes(self.read_bytes()?))
    }

    pub(super) fn read_usize(&mut self) -> Result<u64, LoadError> {
        match self.class {
            Some(ElfClass::Elf32) => Ok(self.read_u32()? as _),
            Some(ElfClass::Elf64) => Ok(self.read_u64()?),
            None => panic!("parsing usize without setting class"),
        }
    }

    pub(super) fn read_isize(&mut self) -> Result<i64, LoadError> {
        match self.class {
            Some(ElfClass::Elf32) => Ok(self.read_i32()? as _),
            Some(ElfClass::Elf64) => Ok(self.read_i64()?),
            None => panic!("parsing isize without setting class"),
        }
    }

    pub(super) fn skip_padding<const N: usize>(&mut self) -> Result<(), LoadError> {
        self.read_bytes::<N>()?;
        Ok(())
    }

    pub(super) fn read_bytes<const N: usize>(&mut self) -> Result<[u8; N], LoadError> {
        let mut buf = [0; N];
        self.reader.read_exact(&mut buf)?;
        Ok(buf)
    }

    pub(super) fn read_vec(&mut self, size: u64) -> Result<Vec<u8>, LoadError> {
        let mut contents = vec![0; size as _];
        self.reader.read_exact(&mut contents)?;
        Ok(contents)
    }

    pub(super) fn align_with_padding(&mut self, align: u64) -> Result<(), LoadError> {
        let current = self.current_position()?;
        if current % align == 0 {
            return Ok(());
        }
        let bytes_to_pad = align - current % align;
        self.reader.seek(SeekFrom::Current(bytes_to_pad as _))?;
        Ok(())
    }

    pub(super) fn current_position(&mut self) -> Result<u64, LoadError> {
        Ok(self.reader.seek(SeekFrom::Current(0))?)
    }

    pub(super) fn duplicate<'new>(
        &mut self,
        new_reader: &'new mut dyn ReadSeek,
    ) -> ReadCursor<'new> {
        ReadCursor {
            reader: new_reader,
            class: self.class,
        }
    }
}
