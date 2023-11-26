use crate::Bits;
use plink_macros::Display;
use std::io::{Read, Write};

pub trait RawType: Sized {
    fn zero() -> Self;
    fn size(class: impl Into<Bits>) -> usize;
    fn read(class: impl Into<Bits>, reader: &mut dyn Read) -> Result<Self, RawReadError>;
    fn write(&self, class: impl Into<Bits>, writer: &mut dyn Write) -> Result<(), RawWriteError>;
}

impl<const N: usize, T: RawType + Copy> RawType for [T; N] {
    fn zero() -> Self {
        [T::zero(); N]
    }

    fn size(class: impl Into<Bits>) -> usize {
        T::size(class) * N
    }

    fn read(bits: impl Into<Bits>, reader: &mut dyn Read) -> Result<Self, RawReadError> {
        let bits = bits.into();
        let mut items = Vec::new();
        for _ in 0..N {
            items.push(RawReadError::wrap_type::<Self, _>(T::read(bits, reader))?);
        }
        match items.try_into() {
            Ok(items) => Ok(items),
            Err(_) => unreachable!(),
        }
    }

    fn write(&self, bits: impl Into<Bits>, writer: &mut dyn Write) -> Result<(), RawWriteError> {
        let bits = bits.into();
        for item in self {
            RawWriteError::wrap_type::<Self, _>(T::write(item, bits, writer))?;
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
                ) -> Result<Self, RawReadError> {
                    let mut buf = [0; std::mem::size_of::<$int>()];
                    reader.read_exact(&mut buf).map_err(RawReadError::io::<$int>)?;
                    Ok(<$int>::from_le_bytes(buf))
                }

                fn write(
                    &self,
                    _bits: impl Into<Bits>,
                    writer: &mut dyn std::io::Write,
                ) -> Result<(), RawWriteError> {
                    writer.write_all(&self.to_le_bytes()).map_err(RawWriteError::io::<$int>)
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

    fn read(_bits: impl Into<Bits>, reader: &mut dyn Read) -> Result<Self, RawReadError> {
        let mut buf = [0; N];
        reader
            .read_exact(&mut buf)
            .map_err(RawReadError::io::<Self>)?;
        Ok(Self)
    }

    fn write(&self, _bits: impl Into<Bits>, writer: &mut dyn Write) -> Result<(), RawWriteError> {
        writer.write_all(&[0; N]).map_err(RawWriteError::io::<Self>)
    }
}

pub trait RawTypeAsPointerSize: Sized {
    fn zero() -> Self;
    fn size(bits: impl Into<Bits>) -> usize;
    fn read(bits: impl Into<Bits>, reader: &mut dyn Read) -> Result<Self, RawReadError>;
    fn write(&self, bits: impl Into<Bits>, writer: &mut dyn Write) -> Result<(), RawWriteError>;
}

macro_rules! impl_rawtypeaspointersize_for_int {
    ($($int:ident or $smallint:ident),*) => {
        $(
            impl RawTypeAsPointerSize for $int {
                fn zero() -> Self {
                    0
                }

                fn size(bits: impl Into<Bits>) -> usize {
                    match bits.into() {
                        Bits::Bits32 => 4,
                        Bits::Bits64 => 8,
                    }
                }

                fn read(bits: impl Into<Bits>, reader: &mut dyn Read) -> Result<Self, RawReadError> {
                    let bits = bits.into();
                    match bits {
                        Bits::Bits32 => <$smallint as RawType>::read(bits, reader).map(|v| v as _),
                        Bits::Bits64 => <$int as RawType>::read(bits, reader),
                    }
                }

                fn write(&self, bits: impl Into<Bits>, writer: &mut dyn Write) -> Result<(), RawWriteError> {
                    let bits = bits.into();
                    match bits {
                        Bits::Bits32 => <$smallint as RawType>::write(&(*self as _), bits, writer),
                        Bits::Bits64 => <$int as RawType>::write(self, bits, writer),
                    }
                }
            }
        )*
    }
}

impl_rawtypeaspointersize_for_int!(i64 or i32, u64 or u32);

#[derive(Debug, Display)]
#[display("failed to read {source}")]
pub struct RawReadError {
    source: ErrorSource,
    inner: RawReadErrorInner,
}

impl RawReadError {
    pub fn io<T>(err: std::io::Error) -> Self {
        Self {
            source: ErrorSource::Type(std::any::type_name::<T>()),
            inner: RawReadErrorInner::IO(err),
        }
    }

    pub fn wrap_type<T, R>(result: Result<R, RawReadError>) -> Result<R, RawReadError> {
        match result {
            Ok(ok) => Ok(ok),
            Err(err) => Err(RawReadError {
                source: ErrorSource::Type(std::any::type_name::<T>()),
                inner: RawReadErrorInner::Itself(Box::new(err)),
            }),
        }
    }

    pub fn wrap_field<T, R>(
        field: &'static str,
        result: Result<R, RawReadError>,
    ) -> Result<R, RawReadError> {
        match result {
            Ok(ok) => Ok(ok),
            Err(err) => Err(RawReadError {
                source: ErrorSource::StructField {
                    field,
                    struct_: std::any::type_name::<T>(),
                },
                inner: RawReadErrorInner::Itself(Box::new(err)),
            }),
        }
    }
}

#[derive(Debug)]
enum RawReadErrorInner {
    Itself(Box<RawReadError>),
    IO(std::io::Error),
}

impl std::error::Error for RawReadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.inner {
            RawReadErrorInner::IO(io) => Some(io),
            RawReadErrorInner::Itself(itself) => Some(itself),
        }
    }
}

#[derive(Debug, Display)]
#[display("failed to write {source}")]
pub struct RawWriteError {
    source: ErrorSource,
    inner: RawWriteErrorInner,
}

impl RawWriteError {
    pub fn io<T>(err: std::io::Error) -> Self {
        Self {
            source: ErrorSource::Type(std::any::type_name::<T>()),
            inner: RawWriteErrorInner::IO(err),
        }
    }

    pub fn wrap_type<T, R>(result: Result<R, RawWriteError>) -> Result<R, RawWriteError> {
        match result {
            Ok(ok) => Ok(ok),
            Err(err) => Err(RawWriteError {
                source: ErrorSource::Type(std::any::type_name::<T>()),
                inner: RawWriteErrorInner::Itself(Box::new(err)),
            }),
        }
    }

    pub fn wrap_field<T, R>(
        field: &'static str,
        result: Result<R, RawWriteError>,
    ) -> Result<R, RawWriteError> {
        match result {
            Ok(ok) => Ok(ok),
            Err(err) => Err(RawWriteError {
                source: ErrorSource::StructField {
                    field,
                    struct_: std::any::type_name::<T>(),
                },
                inner: RawWriteErrorInner::Itself(Box::new(err)),
            }),
        }
    }
}

impl std::error::Error for RawWriteError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.inner {
            RawWriteErrorInner::Itself(itself) => Some(itself),
            RawWriteErrorInner::IO(io) => Some(io),
        }
    }
}

#[derive(Debug)]
enum RawWriteErrorInner {
    Itself(Box<RawWriteError>),
    IO(std::io::Error),
}

#[derive(Debug, Display)]
enum ErrorSource {
    #[display("{f0}")]
    Type(&'static str),
    #[display("field {field} of struct {struct_}")]
    StructField {
        field: &'static str,
        struct_: &'static str,
    },
}
