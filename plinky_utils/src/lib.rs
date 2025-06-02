#![feature(random)]

extern crate proc_macro;

pub mod bitfields;
pub mod filters_parser;
pub mod ints;
mod jaro_similarity;
mod posix_shell;
pub mod quote;
pub mod raw_types;
mod tempdir;

pub use crate::jaro_similarity::jaro_similarity;
pub use crate::tempdir::create_temp_dir;
pub use crate::posix_shell::posix_shell_quote;

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

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum OsAbi {
    SystemV,
    Gnu,
}
