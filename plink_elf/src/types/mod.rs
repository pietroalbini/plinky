pub mod ids;
mod string_table;

pub use self::string_table::ElfStringTable;

use crate::errors::{LoadError, WriteError};
use crate::ids::{convert, ConvertibleElfIds, StringIdGetters};
use crate::reader::{read_object, PendingIds, ReadCursor};
use crate::types::ids::ElfIds;
use crate::utils::{render_hex, ReadSeek, WriteSeek};
use crate::writer::Writer;
use std::collections::BTreeMap;
use std::num::NonZeroU64;
use std::ops::Deref;

#[derive(Debug)]
pub struct ElfObject<I: ElfIds> {
    pub env: ElfEnvironment,
    pub type_: ElfType,
    pub entry: Option<NonZeroU64>,
    pub flags: u32,
    pub sections: BTreeMap<I::SectionId, ElfSection<I>>,
    pub segments: Vec<ElfSegment<I>>,
}

impl<I: ElfIds> ElfObject<I> {
    pub fn load(reader: &mut dyn ReadSeek, ids: &mut I) -> Result<Self, LoadError>
    where
        I: ConvertibleElfIds<PendingIds>,
    {
        let mut cursor = ReadCursor::new(reader);
        let object = read_object(&mut cursor)?;
        Ok(convert(ids, object))
    }

    pub fn write(&self, write_to: &mut dyn WriteSeek) -> Result<(), WriteError>
    where
        I::StringId: StringIdGetters<I>,
    {
        let writer = Writer::new(write_to, self)?;
        writer.write()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ElfEnvironment {
    pub class: ElfClass,
    pub endian: ElfEndian,
    pub abi: ElfABI,
    pub machine: ElfMachine,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElfClass {
    Elf32,
    Elf64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElfABI {
    SystemV,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElfEndian {
    Little,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElfType {
    Relocatable,
    Executable,
    SharedObject,
    Core,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElfMachine {
    X86,
    X86_64,
}

#[derive(Debug)]
pub struct ElfSection<I: ElfIds> {
    pub name: I::StringId,
    pub memory_address: u64,
    pub content: ElfSectionContent<I>,
}

#[derive(Debug)]
pub enum ElfSectionContent<I: ElfIds> {
    Null,
    Program(ElfProgramSection),
    SymbolTable(ElfSymbolTable<I>),
    StringTable(ElfStringTable),
    RelocationsTable(ElfRelocationsTable<I>),
    Note(ElfNotesTable),
    Unknown(ElfUnknownSection),
}

#[derive(Debug)]
pub struct ElfProgramSection {
    pub perms: ElfPermissions,
    pub raw: RawBytes,
}

#[derive(Debug)]
pub struct ElfNotesTable {
    pub notes: Vec<ElfNote>,
}

#[derive(Debug)]
pub struct ElfNote {
    pub name: String,
    pub value: RawBytes,
    pub type_: u32,
}

#[derive(Debug)]
pub struct ElfUnknownSection {
    pub id: u32,
    pub raw: RawBytes,
}

#[derive(Debug)]
pub struct ElfSymbolTable<I: ElfIds> {
    pub symbols: BTreeMap<I::SymbolId, ElfSymbol<I>>,
}

#[derive(Debug)]
pub struct ElfSymbol<I: ElfIds> {
    pub name: I::StringId,
    pub binding: ElfSymbolBinding,
    pub type_: ElfSymbolType,
    pub definition: ElfSymbolDefinition<I>,
    pub value: u64,
    pub size: u64,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ElfSymbolBinding {
    Local,
    Global,
    Weak,
    Unknown(u8),
}

#[derive(Debug)]
pub enum ElfSymbolType {
    NoType,
    Object,
    Function,
    Section,
    File,
    Unknown(u8),
}

#[derive(Debug)]
pub enum ElfSymbolDefinition<I: ElfIds> {
    Undefined,
    Absolute,
    Common,
    Section(I::SectionId),
}

#[derive(Debug)]
pub struct ElfRelocationsTable<I: ElfIds> {
    pub symbol_table: I::SectionId,
    pub applies_to_section: I::SectionId,
    pub relocations: Vec<ElfRelocation<I>>,
}

#[derive(Debug)]
pub struct ElfRelocation<I: ElfIds> {
    pub offset: u64,
    pub symbol: I::SymbolId,
    pub relocation_type: ElfRelocationType,
    pub addend: Option<i64>,
}

#[derive(Debug, Clone, Copy)]
#[allow(non_camel_case_types)]
pub enum ElfRelocationType {
    // x86
    X86_None,
    X86_32,
    X86_PC32,
    // x86_64
    X86_64_None,
    X86_64_64,
    X86_64_PC32,
    X86_64_GOT32,
    X86_64_PLT32,
    X86_64_Copy,
    X86_64_GlobDat,
    X86_64_JumpSlot,
    X86_64_Relative,
    X86_64_GOTPCRel,
    X86_64_32,
    X86_64_32S,
    X86_64_16,
    X86_64_PC16,
    X86_64_8,
    X86_64_PC8,
    X86_64_DTPMod64,
    X86_64_DTPOff64,
    X86_64_TPOff64,
    X86_64_TLSGD,
    X86_64_TLSLD,
    X86_64_DTPOff32,
    X86_64_GOTTPOff,
    X86_64_TPOff32,
    X86_64_PC64,
    X86_64_GOTOff64,
    X86_64_GOTPC32,
    X86_64_Size32,
    X86_64_Size64,
    X86_64_GOTPC32_TLSDesc,
    X86_64_TLSDescCall,
    X86_64_TLSDesc,
    X86_64_IRelative,
    // Other:
    Unknown(u32),
}

#[derive(Debug)]
pub struct ElfSegment<I: ElfIds> {
    pub type_: ElfSegmentType,
    pub perms: ElfPermissions,
    pub content: Vec<ElfSegmentContent<I>>,
    pub align: u64,
}

#[derive(Debug, Clone, Copy)]
pub enum ElfSegmentType {
    Null,
    Load,
    Dynamic,
    Interpreter,
    Note,
    ProgramHeaderTable,
    Unknown(u32),
}

#[derive(Debug, Clone, Copy)]
pub enum ElfSegmentContent<I: ElfIds> {
    Section(I::SectionId),
    Unknown(ElfUnknownSegmentContent),
}

#[derive(Debug, Clone, Copy)]
pub struct ElfUnknownSegmentContent {
    pub file_offset: u64,
    pub virtual_address: u64,
    pub file_size: u64,
    pub memory_size: u64,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct ElfPermissions {
    pub read: bool,
    pub write: bool,
    pub execute: bool,
}

impl std::fmt::Debug for ElfPermissions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut content = String::new();
        if self.read {
            content.push('R');
        }
        if self.write {
            content.push('W');
        }
        if self.execute {
            content.push('X');
        }
        write!(f, "ElfPermissions({content})")
    }
}

pub struct RawBytes(pub Vec<u8>);

impl std::fmt::Debug for RawBytes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RawBytes {")?;
        if self.0.is_empty() {
            // Nothing
        } else if f.alternate() {
            render_hex(f, "    ", &self.0)?;
        } else {
            for byte in &self.0 {
                f.write_fmt(format_args!(" {byte:0>2x}"))?;
            }
            f.write_str(" ")?;
        }
        f.write_str("}")
    }
}

impl Deref for RawBytes {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &*self.0
    }
}
