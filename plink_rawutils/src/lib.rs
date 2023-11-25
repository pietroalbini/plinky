mod raw_types;

pub use raw_types::*;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Bits {
    Bits32,
    Bits64,
}
