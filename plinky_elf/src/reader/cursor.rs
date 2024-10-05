use crate::errors::LoadError;
use crate::{ElfClass, ElfEndian};
use plinky_utils::raw_types::RawType;
use std::io::{Read, Seek, SeekFrom};

pub(crate) struct ReadCursor<'a> {
    reader: InnerReader<'a>,
    pub(crate) class: ElfClass,
    pub(crate) endian: ElfEndian,
}

impl<'a> ReadCursor<'a> {
    pub(crate) fn new(reader: &'a mut dyn ReadSeek, class: ElfClass, endian: ElfEndian) -> Self {
        Self { reader: InnerReader::Borrowed(reader), class, endian }
    }

    pub(crate) fn new_owned(reader: Box<dyn ReadSeek>, class: ElfClass, endian: ElfEndian) -> Self {
        Self { reader: InnerReader::Owned(reader), class, endian }
    }

    pub(super) fn seek_to(&mut self, position: u64) -> Result<(), LoadError> {
        self.reader.get().seek(SeekFrom::Start(position))?;
        Ok(())
    }

    pub(super) fn read_vec(&mut self, size: u64) -> Result<Vec<u8>, LoadError> {
        let mut contents = vec![0; size as _];
        self.reader.get().read_exact(&mut contents)?;
        Ok(contents)
    }

    pub(super) fn read_raw<T: RawType>(&mut self) -> Result<T, LoadError> {
        Ok(T::read(self.class, self.endian, self)?)
    }

    pub(super) fn align_with_padding(&mut self, align: u64) -> Result<(), LoadError> {
        let current = self.current_position()?;
        if current % align == 0 {
            return Ok(());
        }
        let bytes_to_pad = align - current % align;
        self.reader.get().seek(SeekFrom::Current(bytes_to_pad as _))?;
        Ok(())
    }

    pub(super) fn current_position(&mut self) -> Result<u64, LoadError> {
        Ok(self.reader.get().stream_position()?)
    }
}

impl std::io::Read for ReadCursor<'_> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.reader.get().read(buf)
    }
}

enum InnerReader<'a> {
    Borrowed(&'a mut dyn ReadSeek),
    Owned(Box<dyn ReadSeek>),
}

impl<'a> InnerReader<'a> {
    fn get(&mut self) -> &mut dyn ReadSeek {
        match self {
            InnerReader::Borrowed(borrowed) => borrowed,
            InnerReader::Owned(owned) => &mut *owned,
        }
    }
}

pub trait ReadSeek: Read + Seek {}

impl<T: Read + Seek> ReadSeek for T {}
