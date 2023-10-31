mod string_table;

pub use self::string_table::StringTable;

use crate::errors::LoadError;
use crate::reader::{read_object, Cursor};
use crate::utils::{render_hex, ReadSeek};
use std::num::NonZeroU64;
use std::ops::Deref;

#[derive(Debug)]
pub struct Object {
    pub class: Class,
    pub endian: Endian,
    pub abi: ABI,
    pub type_: Type,
    pub machine: Machine,
    pub entry: Option<NonZeroU64>,
    pub flags: u32,
    pub sections: Vec<Section>,
    pub segments: Vec<Segment>,
}

impl Object {
    pub fn load(reader: &mut dyn ReadSeek) -> Result<Self, LoadError> {
        let mut cursor = Cursor::new(reader);
        read_object(&mut cursor)
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Class {
    Elf32,
    Elf64,
}

#[derive(Debug, Clone, Copy)]
pub enum ABI {
    SystemV,
}

#[derive(Debug, Clone, Copy)]
pub enum Endian {
    Little,
    Big,
}

#[derive(Debug, Clone, Copy)]
pub enum Type {
    Relocatable,
    Executable,
    SharedObject,
    Core,
}

#[derive(Debug, Clone, Copy)]
pub enum Machine {
    X86,
    X86_64,
}

#[derive(Debug)]
pub struct Section<S = String> {
    pub name: S,
    pub writeable: bool,
    pub allocated: bool,
    pub executable: bool,
    pub memory_address: u64,
    pub content: SectionContent<S>,
}

#[derive(Debug)]
pub enum SectionContent<S = String> {
    Null,
    Program(ProgramSection),
    SymbolTable(SymbolTable<S>),
    StringTable(StringTable),
    RelocationsTable(RelocationsTable<S>),
    Note(NoteSection),
    Unknown(UnknownSection),
}

#[derive(Debug)]
pub struct ProgramSection {
    pub raw: RawBytes,
}

#[derive(Debug)]
pub struct NoteSection {
    pub raw: RawBytes,
}

#[derive(Debug)]
pub struct UnknownSection {
    pub id: u32,
    pub raw: RawBytes,
}

#[derive(Debug)]
pub struct SymbolTable<S = String> {
    pub symbols: Vec<Symbol<S>>,
}

#[derive(Debug)]
pub struct Symbol<S = String> {
    pub name: S,
    pub binding: SymbolBinding,
    pub type_: SymbolType,
    pub definition: SymbolDefinition,
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
pub enum SymbolDefinition {
    Undefined,
    Absolute,
    Common,
    Section(u16),
}

#[derive(Debug)]
pub struct RelocationsTable<S = String> {
    pub symbol_table: S,
    pub applies_to_section: S,
    pub relocations: Vec<Relocation>,
}

#[derive(Debug)]
pub struct Relocation {
    pub offset: u64,
    pub symbol: u32,
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
