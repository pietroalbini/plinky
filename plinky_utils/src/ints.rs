macro_rules! int {
    ($vis:vis struct $name:ident($inner:ty) from $($from:ty),*) => {
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

        impl From<$inner> for $name {
            fn from(value: $inner) -> Self {
                Self(value)
            }
        }

        $(
            impl From<$from> for $name {
                fn from(value: $from) -> Self {
                    Self(value.into())
                }
            }
        )*
    }
}

int!(pub struct Absolute(u64) from u8, u16, u32);

int!(pub struct Address(u64) from u8, u16, u32);

impl Address {
    pub fn align(&self, align: u64) -> Result<Address, OutOfBoundsError> {
        let delta = self.0 % align;
        if delta == 0 {
            Ok(*self)
        } else {
            Ok(Address(
                self.0
                    .checked_add(align)
                    .ok_or(OutOfBoundsError)?
                    .checked_sub(delta)
                    .ok_or(OutOfBoundsError)?,
            ))
        }
    }

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

int!(pub struct Offset(i64) from u8, i8, u16, i16, u32, i32);

impl Offset {
    pub fn add(&self, other: Offset) -> Result<Offset, OutOfBoundsError> {
        Ok(Offset(self.0.checked_add(other.0).ok_or(OutOfBoundsError)?))
    }

    pub fn neg(&self) -> Offset {
        Offset(-self.0)
    }
}

int!(pub struct Length(u64) from u8, u16, u32);

impl Length {
    pub fn add(&self, other: Length) -> Result<Length, OutOfBoundsError> {
        Ok(Length(self.0.checked_add(other.0).ok_or(OutOfBoundsError)?))
    }

    pub fn as_offset(&self) -> Result<Offset, OutOfBoundsError> {
        Ok(i64::try_from(self.0).map_err(|_| OutOfBoundsError)?.into())
    }
}

impl From<usize> for Length {
    fn from(value: usize) -> Self {
        Self(value as u64)
    }
}

pub trait ExtractNumber {
    type Type;

    fn extract(&self) -> Self::Type;
}

#[derive(Debug, PartialEq, Eq)]
pub struct OutOfBoundsError;

impl std::error::Error for OutOfBoundsError {}

impl std::fmt::Display for OutOfBoundsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("out of bounds math")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_address_align() {
        let a = |n| Address::from(n as u64);
        assert_eq!(a(0x1000), a(0x1000).align(0x1000).unwrap());
        assert_eq!(a(0x2000), a(0x1001).align(0x1000).unwrap());
        assert_eq!(a(9), a(8).align(3).unwrap());
    }
}
