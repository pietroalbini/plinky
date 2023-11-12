use crate::writer::WriteLayoutError;
use crate::ElfABI;
use plink_macros::{Display, Error};

#[derive(Debug, Error, Display)]
pub enum LoadError {
    #[display("I/O error")]
    IO(#[from] std::io::Error),
    #[display("bad ELF magic number: {f0:?}")]
    BadMagic([u8; 4]),
    #[display("bad ELF class: {f0}")]
    BadClass(u8),
    #[display("bad ELF endianness: {f0}")]
    BadEndian(u8),
    #[display("bad ELF version: {f0}")]
    BadVersion(u32),
    #[display("bad ELF ABI: {f0}")]
    BadAbi(u8),
    #[display("bad ELF ABI version: {f0:?}")]
    BadAbiVersion(ElfABI, u8),
    #[display("bad ELF type: {f0}")]
    BadType(u16),
    #[display("bad ELF machine: {f0}")]
    BadMachine(u16),
    #[display("unterminated string")]
    UnterminatedString,
    #[display("non-UTF-8 string")]
    NonUtf8String(#[from] std::string::FromUtf8Error),
    #[display("there is no string table in section {f0}")]
    MissingStringTable(u16),
    #[display("the type of section {f0} is not a string table")]
    WrongStringTableType(u16),
    #[display("missing string with offset {f1:#x} in table {f0}")]
    MissingString(u16, u32),
    #[display("missing section with id {f0:#x}")]
    MissingSection(u16),
    #[display("misaligned file: parsed until {current:#x}, expected to be at {expected:#x}")]
    MisalignedFile { current: usize, expected: usize },
}

#[derive(Debug, Error, Display)]
pub enum WriteError {
    #[display("I/O error")]
    IO(#[from] std::io::Error),
    #[display("missing section names table")]
    MissingSectionNamesTable,
    #[display("different sections point to different string tables for their name")]
    InconsistentSectionNamesTableId,
    #[display("failed to calculate the resulting ELF layout")]
    LayoutError(#[from] WriteLayoutError),
}
