use crate::ids::ElfIds;
use crate::writer::LayoutError;
use crate::ElfABI;
use plinky_macros::{Display, Error};
use plinky_utils::bitfields::BitfieldReadError;

#[derive(Debug, Error, Display)]
pub enum LoadError {
    #[transparent]
    IO(std::io::Error),
    #[transparent]
    RawRead(plinky_utils::raw_types::RawReadError),
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
    #[transparent]
    NonUtf8String(std::string::FromUtf8Error),
    #[display("there is no string table in section {f0}")]
    MissingStringTable(u16),
    #[display("the type of section {f0} is not a string table")]
    WrongStringTableType(u16),
    #[display("missing string with offset {f1:#x} in table {f0}")]
    MissingString(u16, u32),
    #[display("missing section with id {f0:#x}")]
    MissingSection(u16),
    #[display("SHF_INFO_LINK flag set for section {f0} even though it's not a relocation")]
    UnsupportedInfoLinkFlag(u32),
    #[display("only strings with char size of 1 are supported, while section {section_idx} has size {size} (due to SHF_STRINGS)")]
    UnsupportedStringsWithSizeNotOne { section_idx: u32, size: u64 },
    #[display("flag SHF_STRINGS is only expected in sections with SHF_MERGE or in string tables")]
    UnexpectedStringsFlag { section_idx: u32 },
    #[display("flag SHF_MERGE for fixed-sized chunks was provided with chunk size zero on section {section_idx}")]
    FixedSizeChunksMergeWithZeroLenChunks { section_idx: u32 },
    #[display("flag SHF_MERGE was applied on an unsupported section (section {section_idx})")]
    MergeFlagOnUnsupportedSection { section_idx: u32 },
    #[display("bad symbol visibility: {f0}")]
    BadSymbolVisibility(u8),
    #[display("failed to parse the flags field of the dynamic table")]
    DynamicFlags(BitfieldReadError),
    #[display("failed to parse the flags1 field of the dynamic table")]
    DynamicFlags1(BitfieldReadError),
    #[display("failed to parse section header number {idx}")]
    FailedToParseSectionHeader {
        idx: u32,
        #[source]
        inner: Box<LoadError>,
    },
    #[display("failed to parse section number {idx}")]
    FailedToParseSection {
        idx: u16,
        #[source]
        inner: Box<LoadError>,
    },
    #[display("misaligned file: parsed until {current:#x}, expected to be at {expected:#x}")]
    MisalignedFile { current: usize, expected: usize },
}

#[derive(Debug, Error, Display)]
pub enum WriteError<I: ElfIds> {
    #[transparent]
    IO(std::io::Error),
    #[display("failed to write data")]
    RawWrite(#[from] plinky_utils::raw_types::RawWriteError),
    #[display("missing section names table")]
    MissingSectionNamesTable,
    #[display("different sections point to different string tables for their name")]
    InconsistentSectionNamesTableId,
    #[display("different symbols point to different string tables for their name")]
    InconsistentSymbolNamesTableId,
    #[display("missing symbol table {symbol_table:?} for relocations table {relocations_table:?}")]
    MissingSymbolTableForRelocations { symbol_table: I::SectionId, relocations_table: I::SectionId },
    #[display("missing symbol {symbol_id:?} for relocation {relocation_idx} in table {relocations_table:?}")]
    MissingSymbolInRelocation {
        symbol_id: I::SymbolId,
        relocations_table: I::SectionId,
        relocation_idx: usize,
    },
    #[display("group {group:?}'s symbol table {symbol_table:?} is not actually a symbol table")]
    WrongSectionTypeForGroupSymbolTable { group: I::SectionId, symbol_table: I::SectionId },
    #[display("group {group:?}'s signature {signature:?} is missing")]
    MissingGroupSignature { group: I::SectionId, signature: I::SymbolId },
    #[display("value {value} in the dynamic table does not fit")]
    DynamicValueDoesNotFit { value: u64 },
    #[display("failed to calculate the resulting ELF layout")]
    LayoutError(#[from] LayoutError),
}
