pub mod bitfields;
pub mod raw_types;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Bits {
    Bits32,
    Bits64,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Endian {
    Big,
    Little,
}
