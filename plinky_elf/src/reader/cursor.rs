use crate::errors::LoadError;
use crate::{ElfABI, ElfClass, ElfEndian};
use plinky_utils::Bits;
use plinky_utils::raw_types::{RawType, RawTypeContext};
use std::io::{Read, Seek, SeekFrom};

pub(super) struct ReadCursor<'a> {
    reader: InnerReader<'a>,
    raw_ctx: RawTypeContext,
}

impl<'a> ReadCursor<'a> {
    pub(super) fn new(reader: &'a mut dyn ReadSeek, raw_ctx: RawTypeContext) -> Self {
        Self { reader: InnerReader::Borrowed(reader), raw_ctx }
    }

    pub(super) fn new_owned(reader: Box<dyn ReadSeek>, raw_ctx: RawTypeContext) -> Self {
        Self { reader: InnerReader::Owned(reader), raw_ctx }
    }

    pub(super) fn seek_to(&mut self, position: u64) -> Result<(), LoadError> {
        self.reader.get().seek(SeekFrom::Start(position))?;
        Ok(())
    }

    pub(super) fn skip(&mut self, count: u64) -> Result<(), LoadError> {
        self.reader.get().seek_relative(count as i64)?;
        Ok(())
    }

    pub(super) fn read_vec(&mut self, size: u64) -> Result<Vec<u8>, LoadError> {
        let mut contents = vec![0; size as _];
        self.reader.get().read_exact(&mut contents)?;
        Ok(contents)
    }

    pub(super) fn read_raw<T: RawType>(&mut self) -> Result<T, LoadError> {
        Ok(T::read(self.raw_ctx, self)?)
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

    pub(super) fn bits(&self) -> Bits {
        self.raw_ctx.bits
    }

    pub(super) fn set_class(&mut self, class: ElfClass) {
        self.raw_ctx.bits = class.into();
    }

    pub(super) fn set_endian(&mut self, endian: ElfEndian) {
        self.raw_ctx.endian = endian.into();
    }

    pub(super) fn set_os_abi(&mut self, abi: ElfABI) {
        self.raw_ctx.os_abi = abi.into();
    }

    pub(super) fn raw_type_ctx(&self) -> RawTypeContext {
        self.raw_ctx
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
