use crate::errors::LoadError;
use crate::utils::ReadSeek;
use crate::{Class, Endian};
use std::io::SeekFrom;

pub(crate) struct Cursor<'a> {
    reader: &'a mut dyn ReadSeek,
    pub(super) class: Option<Class>,
    pub(super) endian: Option<Endian>,
}

impl<'a> Cursor<'a> {
    pub(crate) fn new(reader: &'a mut dyn ReadSeek) -> Self {
        Self {
            reader,
            class: None,
            endian: None,
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
        let bytes = self.read_bytes()?;
        match self.endian {
            Some(Endian::Big) => Ok(u16::from_be_bytes(bytes)),
            Some(Endian::Little) => Ok(u16::from_le_bytes(bytes)),
            None => panic!("parsing non-u8 without setting endian"),
        }
    }

    pub(super) fn read_u32(&mut self) -> Result<u32, LoadError> {
        let bytes = self.read_bytes()?;
        match self.endian {
            Some(Endian::Big) => Ok(u32::from_be_bytes(bytes)),
            Some(Endian::Little) => Ok(u32::from_le_bytes(bytes)),
            None => panic!("parsing non-u8 without setting endian"),
        }
    }

    pub(super) fn read_u64(&mut self) -> Result<u64, LoadError> {
        let bytes = self.read_bytes()?;
        match self.endian {
            Some(Endian::Big) => Ok(u64::from_be_bytes(bytes)),
            Some(Endian::Little) => Ok(u64::from_le_bytes(bytes)),
            None => panic!("parsing non-u8 without setting endian"),
        }
    }

    pub(super) fn read_i32(&mut self) -> Result<i32, LoadError> {
        let bytes = self.read_bytes()?;
        match self.endian {
            Some(Endian::Big) => Ok(i32::from_be_bytes(bytes)),
            Some(Endian::Little) => Ok(i32::from_le_bytes(bytes)),
            None => panic!("parsing non-u8 without setting endian"),
        }
    }

    pub(super) fn read_i64(&mut self) -> Result<i64, LoadError> {
        let bytes = self.read_bytes()?;
        match self.endian {
            Some(Endian::Big) => Ok(i64::from_be_bytes(bytes)),
            Some(Endian::Little) => Ok(i64::from_le_bytes(bytes)),
            None => panic!("parsing non-u8 without setting endian"),
        }
    }

    pub(super) fn read_usize(&mut self) -> Result<u64, LoadError> {
        match self.class {
            Some(Class::Elf32) => Ok(self.read_u32()? as _),
            Some(Class::Elf64) => Ok(self.read_u64()?),
            None => panic!("parsing usize without setting class"),
        }
    }

    pub(super) fn read_isize(&mut self) -> Result<i64, LoadError> {
        match self.class {
            Some(Class::Elf32) => Ok(self.read_i32()? as _),
            Some(Class::Elf64) => Ok(self.read_i64()?),
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

    pub(super) fn read_vec_at(&mut self, offset: u64, size: u64) -> Result<Vec<u8>, LoadError> {
        self.seek_to(offset)?;
        let mut contents = vec![0; size as _];
        self.reader.read_exact(&mut contents)?;
        Ok(contents)
    }

    pub(super) fn current_position(&mut self) -> Result<u64, LoadError> {
        Ok(self.reader.seek(SeekFrom::Current(0))?)
    }

    pub(super) fn duplicate<'new>(&mut self, new_reader: &'new mut dyn ReadSeek) -> Cursor<'new> {
        Cursor {
            reader: new_reader,
            class: self.class,
            endian: self.endian,
        }
    }
}
