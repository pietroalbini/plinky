#![feature(random)]

extern crate proc_macro;

pub mod bitfields;
pub mod filters_parser;
pub mod ints;
pub mod quote;
pub mod raw_types;
mod jaro_similarity;
mod tempdir;

pub use crate::tempdir::create_temp_dir;
pub use crate::jaro_similarity::jaro_similarity;

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
