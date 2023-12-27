use crate::errors::LoadError;
use crate::utils::ReadSeek;
use crate::{ElfClass, ElfEndian};
use plink_utils::raw_types::RawType;
use std::io::SeekFrom;

pub(crate) struct ReadCursor<'a> {
    reader: &'a mut dyn ReadSeek,
    pub(crate) class: ElfClass,
    pub(crate) endian: ElfEndian,
}

impl<'a> ReadCursor<'a> {
    pub(crate) fn new(reader: &'a mut dyn ReadSeek, class: ElfClass, endian: ElfEndian) -> Self {
        Self { reader, class, endian }
    }

    pub(super) fn seek_to(&mut self, position: u64) -> Result<(), LoadError> {
        self.reader.seek(SeekFrom::Start(position))?;
        Ok(())
    }

    pub(super) fn read_vec(&mut self, size: u64) -> Result<Vec<u8>, LoadError> {
        let mut contents = vec![0; size as _];
        self.reader.read_exact(&mut contents)?;
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
        self.reader.seek(SeekFrom::Current(bytes_to_pad as _))?;
        Ok(())
    }

    pub(super) fn current_position(&mut self) -> Result<u64, LoadError> {
        Ok(self.reader.stream_position()?)
    }

    pub(super) fn duplicate<'new>(
        &mut self,
        new_reader: &'new mut dyn ReadSeek,
    ) -> ReadCursor<'new> {
        ReadCursor { reader: new_reader, class: self.class, endian: self.endian }
    }
}

impl std::io::Read for ReadCursor<'_> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.reader.read(buf)
    }
}
