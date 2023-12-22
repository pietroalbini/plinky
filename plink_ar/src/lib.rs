mod reader;

pub use crate::reader::{ArReader, ArReadError};

#[derive(Debug, PartialEq, Eq)]
pub struct ArchiveFile {
    pub name: String,
    pub content: Vec<u8>,
    pub modification_time: u64,
    pub owner_id: u64,
    pub group_id: u64,
    pub mode: u64,
}
