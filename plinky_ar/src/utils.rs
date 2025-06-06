use plinky_utils::Bits;
use plinky_utils::raw_types::{RawReadError, RawType, RawTypeContext, RawWriteError};
use std::io::{Read, Write};

pub(crate) struct RawString<const LEN: usize> {
    pub(crate) value: String,
}

impl<const LEN: usize> RawType for RawString<LEN> {
    fn zero() -> Self {
        Self { value: " ".repeat(LEN) }
    }

    fn size(_bits: Bits) -> usize {
        LEN
    }

    fn read(_ctx: RawTypeContext, reader: &mut dyn Read) -> Result<Self, RawReadError> {
        let mut buf = [0; LEN];
        reader.read_exact(&mut buf).map_err(RawReadError::io::<Self>)?;
        Ok(Self {
            value: std::str::from_utf8(&buf)
                .map_err(|_| RawReadError::custom::<Self>("failed to decode string".into()))?
                .to_string(),
        })
    }

    fn write(&self, _ctx: RawTypeContext, _writer: &mut dyn Write) -> Result<(), RawWriteError> {
        unimplemented!();
    }
}

pub(crate) struct RawStringAsU64<const LEN: usize, const RADIX: u32> {
    pub(crate) value: u64,
}

impl<const LEN: usize, const RADIX: u32> RawType for RawStringAsU64<LEN, RADIX> {
    fn zero() -> Self {
        Self { value: 0 }
    }

    fn size(bits: Bits) -> usize {
        RawString::<LEN>::size(bits)
    }

    fn read(ctx: RawTypeContext, reader: &mut dyn Read) -> Result<Self, RawReadError> {
        let string = RawReadError::wrap_type::<Self, _>(RawString::<LEN>::read(ctx, reader))?;
        let string = string.value.trim_end_matches(' ');
        if string.is_empty() {
            Ok(Self { value: 0 })
        } else {
            Ok(Self {
                value: u64::from_str_radix(string, RADIX).map_err(|_| {
                    RawReadError::custom::<Self>(format!("failed to parse number from {string:?}",))
                })?,
            })
        }
    }

    fn write(&self, _ctx: RawTypeContext, _writer: &mut dyn Write) -> Result<(), RawWriteError> {
        unimplemented!();
    }
}
