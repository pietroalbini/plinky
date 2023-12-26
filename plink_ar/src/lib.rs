mod reader;
mod utils;

pub use crate::reader::{ArReadError, ArReader};
use std::collections::HashMap;

#[derive(Debug, PartialEq, Eq)]
pub enum ArMember {
    SymbolTable(ArSymbolTable),
    File(ArFile),
}

#[derive(Debug, PartialEq, Eq)]
pub struct ArFile {
    pub name: String,
    pub content: Vec<u8>,
    pub modification_time: u64,
    pub owner_id: u64,
    pub group_id: u64,
    pub mode: u64,
}

#[derive(Debug, PartialEq, Eq)]
pub struct ArSymbolTable {
    pub symbols: HashMap<String, ArMemberId>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct ArMemberId {
    reader_serial: u64,
    header_offset: u64,
}
