use crate::errors::LoadError;
use crate::reader::ObjectReader;
use crate::utils::{ReadSeek, render_hex};
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
pub struct Segment {
    pub type_: SegmentType,
    pub flags: u32,
    pub contents: RawBytes,
}

#[derive(Debug, Clone, Copy)]
pub enum SegmentType {
    Null,
    Loadable,
    DynamicLinkingTables,
    ProgramInterpreter,
    Note,
    ProgramHeaderTable,
    Unknown(u32),
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
