mod string_table;

pub use self::string_table::StringTable;

use crate::errors::LoadError;
use crate::reader::ObjectReader;
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
        let reader = ObjectReader::new(reader);
        reader.read()
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
    pub info: u8,
    pub definition: SymbolDefinition,
    pub value: u64,
    pub size: u64,
}

#[derive(Debug)]
pub enum SymbolDefinition {
    Undefined,
    Absolute,
    Common,
    Section(u16),
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
