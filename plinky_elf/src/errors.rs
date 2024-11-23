use crate::ids::{ElfSectionId, ElfSymbolId};
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
    #[display("failed to parse the x86 features 2 used GNU property")]
    X86Features2Used(BitfieldReadError),
    #[display("failed to parse the x86 ISA used GNU property")]
    X86IsaUsed(BitfieldReadError),
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
    #[display("entry size defined in the section metadata is zero")]
    EntrySizeZero,
    #[display("sh_size (value: {len}, hex: {len:#x}) is not a multiple of sh_entsize (value: {entry_len}, hex: {entry_len:#x}) defined in the section header")]
    LenNotMultipleOfEntrySize {
        len: u64,
        entry_len: u64,
    },
    #[display("no section present at address {f0:#x}")]
    NoSectionAtAddress(u64),
}

#[derive(Debug, Error, Display)]
pub enum WriteError {
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
    MissingSymbolTableForRelocations { symbol_table: ElfSectionId, relocations_table: ElfSectionId },
    #[display("group {group:?}'s symbol table {symbol_table:?} is not actually a symbol table")]
    WrongSectionTypeForGroupSymbolTable { group: ElfSectionId, symbol_table: ElfSectionId },
    #[display("group {group:?}'s signature {signature:?} is missing")]
    MissingGroupSignature { group: ElfSectionId, signature: ElfSymbolId },
    #[display("value {value} in the dynamic table does not fit")]
    DynamicValueDoesNotFit { value: u64 },
    #[display("note is too long")]
    NoteTooLong,
    #[display("failed to calculate the resulting ELF layout")]
    LayoutError(#[from] LayoutError),
}
