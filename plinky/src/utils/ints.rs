use plinky_macros::{Display, Error};

macro_rules! int {
    ($vis:vis struct $name:ident($inner:ty)) => {
        #[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
        $vis struct $name($inner);

        impl crate::utils::ints::ExtractNumber for $name {
            type Type = $inner;

            fn extract(&self) -> $inner {
                self.0
            }
        }

        impl std::fmt::Debug for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "{:#x}", self.0)
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "{:#x}", self.0)
            }
        }

        impl<T: Into<$inner>> From<T> for $name {
            fn from(value: T) -> Self {
                Self(value.into())
            }
        }
    }
}

int!(pub(crate) struct Absolute(u64));

int!(pub(crate) struct Address(u64));

impl Address {
    pub(crate) fn offset(&self, offset: Offset) -> Result<Address, OutOfBoundsError> {
        Ok(Address(
            i128::from(self.0)
                .checked_add(i128::from(offset.0))
                .ok_or(OutOfBoundsError)?
                .try_into()
                .map_err(|_| OutOfBoundsError)?,
        ))
    }

    pub(crate) fn as_offset(&self) -> Result<Offset, OutOfBoundsError> {
        Ok(i64::try_from(self.0).map_err(|_| OutOfBoundsError)?.into())
    }

    pub(crate) fn as_absolute(&self) -> Absolute {
        Absolute(self.0)
    }
}

int!(pub(crate) struct Offset(i64));

impl Offset {
    pub(crate) fn len(len: usize) -> Offset {
        Offset(len as _)
    }

    pub(crate) fn add(&self, other: Offset) -> Result<Offset, OutOfBoundsError> {
        Ok(Offset(self.0.checked_add(other.0).ok_or(OutOfBoundsError)?))
    }

    pub(crate) fn neg(&self) -> Offset {
        Offset(-self.0)
    }
}

pub(crate) trait ExtractNumber {
    type Type;

    fn extract(&self) -> Self::Type;
}

#[derive(Debug, Error, Display)]
#[display("out of bounds math")]
pub(crate) struct OutOfBoundsError;
