use crate::ElfClass;
use std::io::{Error, Read, Write};

pub(crate) trait RawType: Sized {
    fn zero() -> Self;
    fn size(class: ElfClass) -> usize;
    fn read(class: ElfClass, reader: &mut dyn Read) -> Result<Self, Error>;
    fn write(&self, class: ElfClass, writer: &mut dyn Write) -> Result<(), Error>;
}

impl<const N: usize, T: RawType + Copy> RawType for [T; N] {
    fn zero() -> Self {
        [T::zero(); N]
    }

    fn size(class: ElfClass) -> usize {
        T::size(class) * N
    }

    fn read(class: ElfClass, reader: &mut dyn Read) -> Result<Self, Error> {
        let mut items = Vec::new();
        for _ in 0..N {
            items.push(T::read(class, reader)?);
        }
        match items.try_into() {
            Ok(items) => Ok(items),
            Err(_) => unreachable!(),
        }
    }

    fn write(&self, class: ElfClass, writer: &mut dyn Write) -> Result<(), Error> {
        for item in self {
            T::write(item, class, writer)?;
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

                fn size(_class: ElfClass) -> usize {
                    std::mem::size_of::<$int>()
                }

                fn read(
                    _class: ElfClass,
                    reader: &mut dyn std::io::Read,
                ) -> Result<Self, std::io::Error> {
                    let mut buf = [0; std::mem::size_of::<$int>()];
                    reader.read_exact(&mut buf)?;
                    Ok(<$int>::from_le_bytes(buf))
                }

                fn write(
                    &self,
                    _class: ElfClass,
                    writer: &mut dyn std::io::Write,
                ) -> Result<(), std::io::Error> {
                    writer.write_all(&self.to_le_bytes())
                }
            }
        )*
    }
}

impl_rawtype_for_int!(u8, u16, u32, u64, i8, i16, i32, i64);

pub(crate) struct RawPadding<const N: usize>;

impl<const N: usize> RawType for RawPadding<N> {
    fn zero() -> Self {
        Self
    }

    fn size(_class: ElfClass) -> usize {
        N
    }

    fn read(_class: ElfClass, reader: &mut dyn Read) -> Result<Self, Error> {
        let mut buf = [0; N];
        reader.read_exact(&mut buf)?;
        Ok(Self)
    }

    fn write(&self, _class: ElfClass, writer: &mut dyn Write) -> Result<(), Error> {
        writer.write_all(&[0; N])
    }
}

pub(crate) trait RawTypeAsPointerSize: Sized {
    fn zero() -> Self;
    fn size(class: ElfClass) -> usize;
    fn read(class: ElfClass, reader: &mut dyn Read) -> Result<Self, Error>;
    fn write(&self, class: ElfClass, writer: &mut dyn Write) -> Result<(), Error>;
}

impl RawTypeAsPointerSize for u64 {
    fn zero() -> Self {
        0
    }

    fn size(class: ElfClass) -> usize {
        match class {
            ElfClass::Elf32 => 4,
            ElfClass::Elf64 => 8,
        }
    }

    fn read(class: ElfClass, reader: &mut dyn Read) -> Result<Self, Error> {
        match class {
            ElfClass::Elf32 => <u32 as RawType>::read(class, reader).map(|v| v as _),
            ElfClass::Elf64 => <u64 as RawType>::read(class, reader),
        }
    }

    fn write(&self, class: ElfClass, writer: &mut dyn Write) -> Result<(), Error> {
        match class {
            ElfClass::Elf32 => <u32 as RawType>::write(&(*self as _), class, writer),
            ElfClass::Elf64 => <u64 as RawType>::write(self, class, writer),
        }
    }
}

impl RawTypeAsPointerSize for i64 {
    fn zero() -> Self {
        0
    }

    fn size(class: ElfClass) -> usize {
        match class {
            ElfClass::Elf32 => 4,
            ElfClass::Elf64 => 8,
        }
    }

    fn read(class: ElfClass, reader: &mut dyn Read) -> Result<Self, Error> {
        match class {
            ElfClass::Elf32 => <i32 as RawType>::read(class, reader).map(|v| v as _),
            ElfClass::Elf64 => <i64 as RawType>::read(class, reader),
        }
    }

    fn write(&self, class: ElfClass, writer: &mut dyn Write) -> Result<(), Error> {
        match class {
            ElfClass::Elf32 => <i32 as RawType>::write(&(*self as _), class, writer),
            ElfClass::Elf64 => <i64 as RawType>::write(self, class, writer),
        }
    }
}
