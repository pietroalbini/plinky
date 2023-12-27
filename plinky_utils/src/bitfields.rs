use crate::raw_types::{RawReadError, RawType, RawTypeAsPointerSize, RawWriteError};
use crate::{Bits, Endian};
use plinky_macros::{Display, Error};
use std::io::{Read, Write};

pub trait Bitfield: Sized {
    type Repr: BitfieldRepr;

    fn read(raw: Self::Repr) -> Result<Self, BitfieldReadError>;
    fn write(&self) -> Self::Repr;
}

impl<T> RawType for T
where
    T: Bitfield,
    T::Repr: RawType,
{
    fn zero() -> Self {
        <T as Bitfield>::read(<T as Bitfield>::Repr::empty())
            .expect("failed to parse empty bitfield")
    }

    fn size(bits: impl Into<Bits>) -> usize {
        T::Repr::size(bits)
    }

    fn read(
        bits: impl Into<Bits>,
        endian: impl Into<Endian>,
        reader: &mut dyn Read,
    ) -> Result<Self, RawReadError> {
        let raw =
            RawReadError::wrap_type::<T, _>(<T::Repr as RawType>::read(bits, endian, reader))?;
        <T as Bitfield>::read(raw).map_err(RawReadError::bitfield::<T>)
    }

    fn write(
        &self,
        bits: impl Into<Bits>,
        endian: impl Into<Endian>,
        writer: &mut dyn Write,
    ) -> Result<(), RawWriteError> {
        let raw = <T as Bitfield>::write(self);
        RawWriteError::wrap_type::<Self, _>(<T::Repr as RawType>::write(&raw, bits, endian, writer))
    }
}

impl<T> RawTypeAsPointerSize for T
where
    T: Bitfield,
    T::Repr: RawTypeAsPointerSize,
{
    fn zero() -> Self {
        <T as Bitfield>::read(<T as Bitfield>::Repr::empty())
            .expect("failed to parse empty bitfield")
    }

    fn size(bits: impl Into<Bits>) -> usize {
        T::Repr::size(bits)
    }

    fn read(
        bits: impl Into<Bits>,
        endian: impl Into<Endian>,
        reader: &mut dyn Read,
    ) -> Result<Self, RawReadError> {
        let raw = RawReadError::wrap_type::<T, _>(<T::Repr as RawTypeAsPointerSize>::read(
            bits, endian, reader,
        ))?;
        <T as Bitfield>::read(raw).map_err(RawReadError::bitfield::<T>)
    }

    fn write(
        &self,
        bits: impl Into<Bits>,
        endian: impl Into<Endian>,
        writer: &mut dyn Write,
    ) -> Result<(), RawWriteError> {
        let raw = <T as Bitfield>::write(self);
        RawWriteError::wrap_type::<Self, _>(<T::Repr as RawTypeAsPointerSize>::write(
            &raw, bits, endian, writer,
        ))
    }
}

pub struct BitfieldReader<R: BitfieldRepr> {
    value: R,
    mask: R,
}

impl<R: BitfieldRepr> BitfieldReader<R> {
    pub fn new(value: R) -> Self {
        Self { value, mask: R::empty() }
    }

    pub fn bit(&mut self, idx: u64) -> bool {
        if idx >= R::MAX_BITS {
            panic!("bitfield cannot fit idx {idx}");
        }
        self.mask.set_bit(idx);
        self.value.is_bit_set(idx)
    }

    pub fn check_for_unknown_bits(&self) -> Result<(), BitfieldReadError> {
        let masked = self.value.and(&self.mask.invert());
        if masked == R::empty() {
            Ok(())
        } else {
            Err(BitfieldReadError::UnknownBit(masked.first_set_bit_idx()))
        }
    }
}

pub struct BitfieldWriter<R: BitfieldRepr> {
    value: R,
}

impl<R: BitfieldRepr> BitfieldWriter<R> {
    pub fn new() -> Self {
        Self { value: R::empty() }
    }

    pub fn set_bit(&mut self, idx: u64, value: bool) {
        if idx >= R::MAX_BITS {
            panic!("bitfield cannot fit idx {idx}");
        }
        if value {
            self.value.set_bit(idx);
        }
    }

    pub fn value(self) -> R {
        self.value
    }
}

pub trait BitfieldRepr: PartialEq {
    const MAX_BITS: u64;

    fn empty() -> Self;

    fn is_bit_set(&self, idx: u64) -> bool;
    fn set_bit(&mut self, idx: u64);
    fn first_set_bit_idx(&self) -> u64;

    fn invert(&self) -> Self;
    fn and(&self, rhs: &Self) -> Self;
}

macro_rules! impl_bitfieldrepr_for {
    ($($ty:ty),*) => {
        $(
            impl BitfieldRepr for $ty {
                const MAX_BITS: u64 = std::mem::size_of::<$ty>() as u64 * 8;

                fn empty() -> Self {
                    0
                }

                fn is_bit_set(&self, idx: u64) -> bool {
                    let mask = 0x1 << idx;
                    *self & mask > 0
                }

                fn set_bit(&mut self, idx: u64) {
                    *self |= 0x1 << idx;
                }

                fn first_set_bit_idx(&self) -> u64 {
                    self.trailing_zeros() as _
                }

                fn and(&self, rhs: &Self) -> Self {
                    *self & *rhs
                }

                fn invert(&self) -> Self {
                    !*self
                }
            }
        )*
    }
}

impl_bitfieldrepr_for!(u8, u16, u32, u64);

#[derive(Debug, Error, Display)]
pub enum BitfieldReadError {
    #[display("unknown bit set to true at position {f0}")]
    UnknownBit(u64),
}
