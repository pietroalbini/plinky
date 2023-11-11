use crate::reader::ReadCursor;
use crate::errors::LoadError;

pub(crate) trait RawType: Sized {
    fn size() -> usize;
    fn read(cursor: &mut ReadCursor<'_>) -> Result<Self, LoadError>;
}

impl<const N: usize, T: RawType> RawType for [T; N] {
    fn size() -> usize {
        T::size() * N
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
}

macro_rules! impl_rawtype_for_int {
    ($($int:ty),*) => {
        $(
            impl RawType for $int {
                fn size() -> usize {
                    std::mem::size_of::<$int>()
                }

                fn read(cursor: &mut ReadCursor<'_>) -> Result<Self, LoadError> {
                    Ok(<$int>::from_le_bytes(cursor.read_bytes()?))
                }
            }
        )*
    }
}

impl_rawtype_for_int!(u8, u16, u32, u64, i8, i16, i32, i64);

pub(crate) struct RawPadding<const N: usize>;

impl<const N: usize> RawType for RawPadding<N> {
    fn size() -> usize {
        N
    }

    fn read(cursor: &mut ReadCursor<'_>) -> Result<Self, LoadError> {
        cursor.skip_padding::<N>()?;
        Ok(RawPadding)
    }
}

pub(crate) trait RawTypeAsPointerSize: RawType {
    fn read(cursor: &mut ReadCursor<'_>) -> Result<Self, LoadError>;
}

impl RawTypeAsPointerSize for u64 {
    fn read(cursor: &mut ReadCursor<'_>) -> Result<Self, LoadError> {
        cursor.read_usize()
    }
}

impl RawTypeAsPointerSize for i64 {
    fn read(cursor: &mut ReadCursor<'_>) -> Result<Self, LoadError> {
        cursor.read_isize()
    }
}
