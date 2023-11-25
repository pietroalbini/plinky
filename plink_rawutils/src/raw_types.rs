use crate::Bits;
use std::io::{Error, Read, Write};

pub trait RawType: Sized {
    fn zero() -> Self;
    fn size(class: impl Into<Bits>) -> usize;
    fn read(class: impl Into<Bits>, reader: &mut dyn Read) -> Result<Self, Error>;
    fn write(&self, class: impl Into<Bits>, writer: &mut dyn Write) -> Result<(), Error>;
}

impl<const N: usize, T: RawType + Copy> RawType for [T; N] {
    fn zero() -> Self {
        [T::zero(); N]
    }

    fn size(class: impl Into<Bits>) -> usize {
        T::size(class) * N
    }

    fn read(bits: impl Into<Bits>, reader: &mut dyn Read) -> Result<Self, Error> {
        let bits = bits.into();
        let mut items = Vec::new();
        for _ in 0..N {
            items.push(T::read(bits, reader)?);
        }
        match items.try_into() {
            Ok(items) => Ok(items),
            Err(_) => unreachable!(),
        }
    }

    fn write(&self, bits: impl Into<Bits>, writer: &mut dyn Write) -> Result<(), Error> {
        let bits = bits.into();
        for item in self {
            T::write(item, bits, writer)?;
        }
        Ok(())
    }
}

macro_rules! impl_rawtype_for_int {
    ($($int:ty),*) => {
        $(
            impl RawType for $int {
                fn zero() -> Self {
                    0
                }

                fn size(_bits: impl Into<Bits>) -> usize {
                    std::mem::size_of::<$int>()
                }

                fn read(
                    _bits: impl Into<Bits>,
                    reader: &mut dyn std::io::Read,
                ) -> Result<Self, std::io::Error> {
                    let mut buf = [0; std::mem::size_of::<$int>()];
                    reader.read_exact(&mut buf)?;
                    Ok(<$int>::from_le_bytes(buf))
                }

                fn write(
                    &self,
                    _bits: impl Into<Bits>,
                    writer: &mut dyn std::io::Write,
                ) -> Result<(), std::io::Error> {
                    writer.write_all(&self.to_le_bytes())
                }
            }
        )*
    }
}

impl_rawtype_for_int!(u8, u16, u32, u64, i8, i16, i32, i64);

pub struct RawPadding<const N: usize>;

impl<const N: usize> RawType for RawPadding<N> {
    fn zero() -> Self {
        Self
    }

    fn size(_bits: impl Into<Bits>) -> usize {
        N
    }

    fn read(_bits: impl Into<Bits>, reader: &mut dyn Read) -> Result<Self, Error> {
        let mut buf = [0; N];
        reader.read_exact(&mut buf)?;
        Ok(Self)
    }

    fn write(&self, _bits: impl Into<Bits>, writer: &mut dyn Write) -> Result<(), Error> {
        writer.write_all(&[0; N])
    }
}

pub trait RawTypeAsPointerSize: Sized {
    fn zero() -> Self;
    fn size(bits: impl Into<Bits>) -> usize;
    fn read(bits: impl Into<Bits>, reader: &mut dyn Read) -> Result<Self, Error>;
    fn write(&self, bits: impl Into<Bits>, writer: &mut dyn Write) -> Result<(), Error>;
}

impl RawTypeAsPointerSize for u64 {
    fn zero() -> Self {
        0
    }

    fn size(bits: impl Into<Bits>) -> usize {
        match bits.into() {
            Bits::Bits32 => 4,
            Bits::Bits64 => 8,
        }
    }

    fn read(bits: impl Into<Bits>, reader: &mut dyn Read) -> Result<Self, Error> {
        let bits = bits.into();
        match bits {
            Bits::Bits32 => <u32 as RawType>::read(bits, reader).map(|v| v as _),
            Bits::Bits64 => <u64 as RawType>::read(bits, reader),
        }
    }

    fn write(&self, bits: impl Into<Bits>, writer: &mut dyn Write) -> Result<(), Error> {
        let bits = bits.into();
        match bits {
            Bits::Bits32 => <u32 as RawType>::write(&(*self as _), bits, writer),
            Bits::Bits64 => <u64 as RawType>::write(self, bits, writer),
        }
    }
}

impl RawTypeAsPointerSize for i64 {
    fn zero() -> Self {
        0
    }

    fn size(bits: impl Into<Bits>) -> usize {
        match bits.into() {
            Bits::Bits32 => 4,
            Bits::Bits64 => 8,
        }
    }

    fn read(bits: impl Into<Bits>, reader: &mut dyn Read) -> Result<Self, Error> {
        let bits = bits.into();
        match bits {
            Bits::Bits32 => <i32 as RawType>::read(bits, reader).map(|v| v as _),
            Bits::Bits64 => <i64 as RawType>::read(bits, reader),
        }
    }

    fn write(&self, bits: impl Into<Bits>, writer: &mut dyn Write) -> Result<(), Error> {
        let bits = bits.into();
        match bits {
            Bits::Bits32 => <i32 as RawType>::write(&(*self as _), bits, writer),
            Bits::Bits64 => <i64 as RawType>::write(self, bits, writer),
        }
    }
}
