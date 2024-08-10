macro_rules! int {
    ($vis:vis struct $name:ident($inner:ty)) => {
        #[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
        $vis struct $name($inner);

        impl $crate::ints::ExtractNumber for $name {
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

int!(pub struct Absolute(u64));

int!(pub struct Address(u64));

impl Address {
    pub fn offset(&self, offset: Offset) -> Result<Address, OutOfBoundsError> {
        Ok(Address(
            i128::from(self.0)
                .checked_add(i128::from(offset.0))
                .ok_or(OutOfBoundsError)?
                .try_into()
                .map_err(|_| OutOfBoundsError)?,
        ))
    }

    pub fn as_offset(&self) -> Result<Offset, OutOfBoundsError> {
        Ok(i64::try_from(self.0).map_err(|_| OutOfBoundsError)?.into())
    }

    pub fn as_absolute(&self) -> Absolute {
        Absolute(self.0)
    }
}

int!(pub struct Offset(i64));

impl Offset {
    pub fn len(len: usize) -> Offset {
        Offset(len as _)
    }

    pub fn add(&self, other: Offset) -> Result<Offset, OutOfBoundsError> {
        Ok(Offset(self.0.checked_add(other.0).ok_or(OutOfBoundsError)?))
    }

    pub fn neg(&self) -> Offset {
        Offset(-self.0)
    }
}

pub trait ExtractNumber {
    type Type;

    fn extract(&self) -> Self::Type;
}

#[derive(Debug)]
pub struct OutOfBoundsError;

impl std::error::Error for OutOfBoundsError {}

impl std::fmt::Display for OutOfBoundsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("out of bounds math")
    }
}
