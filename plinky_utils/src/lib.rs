extern crate proc_macro;

pub mod bitfields;
pub mod filters_parser;
pub mod quote;
pub mod raw_types;
pub mod ints;

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
