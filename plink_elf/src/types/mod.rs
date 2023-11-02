pub mod ids;
mod string_table;

pub use self::string_table::StringTable;

use crate::errors::LoadError;
use crate::ids::{convert, ConvertibleElfIds};
use crate::reader::{read_object, Cursor, PendingIds};
use crate::types::ids::ElfIds;
use crate::utils::{render_hex, ReadSeek};
use std::collections::BTreeMap;
use std::num::NonZeroU64;
use std::ops::Deref;

#[derive(Debug)]
pub struct Object<I: ElfIds> {
    pub env: Environment,
    pub type_: Type,
    pub entry: Option<NonZeroU64>,
    pub flags: u32,
    pub sections: BTreeMap<I::SectionId, Section<I>>,
    pub segments: Vec<Segment>,
}

impl<I: ElfIds> Object<I> {
    pub fn load(reader: &mut dyn ReadSeek, ids: &mut I) -> Result<Self, LoadError>
    where
        I: ConvertibleElfIds<PendingIds>,
    {
        let mut cursor = Cursor::new(reader);
        let object = read_object(&mut cursor)?;
        Ok(convert(ids, object))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Environment {
    pub class: Class,
    pub endian: Endian,
    pub abi: ABI,
    pub machine: Machine,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    Elf32,
    Elf64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ABI {
    SystemV,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Endian {
    Little,
    Big,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Type {
    Relocatable,
    Executable,
    SharedObject,
    Core,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Machine {
    X86,
    X86_64,
}

#[derive(Debug)]
pub struct Section<I: ElfIds> {
    pub name: I::StringId,
    pub memory_address: u64,
    pub content: SectionContent<I>,
}

#[derive(Debug)]
pub enum SectionContent<I: ElfIds> {
    Null,
    Program(ProgramSection),
    SymbolTable(SymbolTable<I>),
    StringTable(StringTable),
    RelocationsTable(RelocationsTable<I>),
    Note(NotesTable),
    Unknown(UnknownSection),
}

#[derive(Debug)]
pub struct ProgramSection {
    pub writeable: bool,
    pub allocated: bool,
    pub executable: bool,
    pub raw: RawBytes,
}

#[derive(Debug)]
pub struct NotesTable {
    pub notes: Vec<Note>,
}

#[derive(Debug)]
pub struct Note {
    pub name: String,
    pub value: RawBytes,
    pub type_: u32,
}

#[derive(Debug)]
pub struct UnknownSection {
    pub id: u32,
    pub raw: RawBytes,
}

#[derive(Debug)]
pub struct SymbolTable<I: ElfIds> {
    pub symbols: BTreeMap<I::SymbolId, Symbol<I>>,
}

#[derive(Debug)]
pub struct Symbol<I: ElfIds> {
    pub name: I::StringId,
    pub binding: SymbolBinding,
    pub type_: SymbolType,
    pub definition: SymbolDefinition<I>,
    pub value: u64,
    pub size: u64,
}

#[derive(Debug)]
pub enum SymbolBinding {
    Local,
    Global,
    Weak,
    Unknown(u8),
}

#[derive(Debug)]
pub enum SymbolType {
    NoType,
    Object,
    Function,
    Section,
    File,
    Unknown(u8),
}

#[derive(Debug)]
pub enum SymbolDefinition<I: ElfIds> {
    Undefined,
    Absolute,
    Common,
    Section(I::SectionId),
}

#[derive(Debug)]
pub struct RelocationsTable<I: ElfIds> {
    pub symbol_table: I::SectionId,
    pub applies_to_section: I::SectionId,
    pub relocations: Vec<Relocation<I>>,
}

#[derive(Debug)]
pub struct Relocation<I: ElfIds> {
    pub offset: u64,
    pub symbol: I::SymbolId,
    pub relocation_type: RelocationType,
    pub addend: Option<i64>,
}

#[derive(Debug)]
#[allow(non_camel_case_types)]
pub enum RelocationType {
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
pub struct Segment {
    pub content: SegmentContent,
}

#[derive(Debug)]
pub enum SegmentContent {
    Unknown { id: u32, raw: RawBytes },
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
