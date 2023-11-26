use crate::ids::ElfIds;
use crate::writer::WriteLayoutError;
use crate::ElfABI;
use plink_macros::{Display, Error};

#[derive(Debug, Error, Display)]
pub enum LoadError {
    #[display("I/O error")]
    IO(#[from] std::io::Error),
    #[display("failed to read data")]
    RawRead(#[from] plink_rawutils::raw_types::RawReadError),
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
pub enum WriteError<I: ElfIds> {
    #[display("I/O error")]
    IO(#[from] std::io::Error),
    #[display("failed to write data")]
    RawWrite(#[from] plink_rawutils::raw_types::RawWriteError),
    #[display("missing section names table")]
    MissingSectionNamesTable,
    #[display("different sections point to different string tables for their name")]
    InconsistentSectionNamesTableId,
    #[display("different symbols point to different string tables for their name")]
    InconsistentSymbolNamesTableId,
    #[display("missing symbol table {symbol_table:?} for relocations table {relocations_table:?}")]
    MissingSymbolTableForRelocations {
        symbol_table: I::SectionId,
        relocations_table: I::SectionId,
    },
    #[display("missing symbol {symbol_id:?} for relocation {relocation_idx} in table {relocations_table:?}")]
    MissingSymbolInRelocation {
        symbol_id: I::SymbolId,
        relocations_table: I::SectionId,
        relocation_idx: usize,
    },
    #[display("failed to calculate the resulting ELF layout")]
    LayoutError(#[from] WriteLayoutError),
}
