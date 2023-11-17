use crate::errors::{LoadError, WriteError};
use crate::reader::ReadCursor;
use crate::writer::WriteCursor;
use crate::ElfClass;

pub(crate) trait RawType: Sized {
    fn zero() -> Self;
    fn size(class: ElfClass) -> usize;
    fn read(cursor: &mut ReadCursor<'_>) -> Result<Self, LoadError>;
    fn write(&self, cursor: &mut WriteCursor<'_>) -> Result<(), WriteError>;
}

impl<const N: usize, T: RawType + Copy> RawType for [T; N] {
    fn zero() -> Self {
        [T::zero(); N]
    }

    fn size(class: ElfClass) -> usize {
        T::size(class) * N
    }

    fn read(cursor: &mut ReadCursor<'_>) -> Result<Self, LoadError> {
        let mut items = Vec::new();
        for _ in 0..N {
            items.push(T::read(cursor)?);
        }
        match items.try_into() {
            Ok(items) => Ok(items),
            Err(_) => unreachable!(),
        }
    }

    fn write(&self, cursor: &mut WriteCursor<'_>) -> Result<(), WriteError> {
        for item in self {
            T::write(item, cursor)?;
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

                fn read(cursor: &mut ReadCursor<'_>) -> Result<Self, LoadError> {
                    Ok(<$int>::from_le_bytes(cursor.read_bytes()?))
                }

                fn write(&self, cursor: &mut WriteCursor<'_>) -> Result<(), WriteError> {
                    cursor.write_bytes(&self.to_le_bytes())
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

    fn read(cursor: &mut ReadCursor<'_>) -> Result<Self, LoadError> {
        cursor.skip_padding::<N>()?;
        Ok(RawPadding)
    }

    fn write(&self, cursor: &mut WriteCursor<'_>) -> Result<(), WriteError> {
        cursor.write_bytes(&[0; N])
    }
}

pub(crate) trait RawTypeAsPointerSize: Sized {
    fn zero() -> Self;
    fn size(class: ElfClass) -> usize;
    fn read(cursor: &mut ReadCursor<'_>) -> Result<Self, LoadError>;
    fn write(&self, cursor: &mut WriteCursor<'_>) -> Result<(), WriteError>;
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

    fn read(cursor: &mut ReadCursor<'_>) -> Result<Self, LoadError> {
        cursor.read_usize()
    }

    fn write(&self, cursor: &mut WriteCursor<'_>) -> Result<(), WriteError> {
        cursor.write_usize(*self)
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

    fn read(cursor: &mut ReadCursor<'_>) -> Result<Self, LoadError> {
        cursor.read_isize()
    }

    fn write(&self, cursor: &mut WriteCursor<'_>) -> Result<(), WriteError> {
        cursor.write_isize(*self)
    }
}
