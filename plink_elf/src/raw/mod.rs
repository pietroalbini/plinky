#[macro_use]
mod support;

pub(crate) use self::support::*;

use crate::errors::{LoadError, WriteError};
use crate::reader::ReadCursor;
use crate::writer::WriteCursor;
use plink_macros::RawType;

#[derive(RawType)]
pub(crate) struct RawIdentification {
    pub(crate) magic: [u8; 4],
    pub(crate) class: u8,
    pub(crate) endian: u8,
    pub(crate) version: u8,
    pub(crate) abi: u8,
    pub(crate) abi_version: u8,
    pub(crate) padding: RawPadding<7>,
}

#[derive(RawType)]
pub(crate) struct RawHeader {
    pub(crate) type_: u16,
    pub(crate) machine: u16,
    pub(crate) version: u32,
    #[pointer_size]
    pub(crate) entry: u64,
    #[pointer_size]
    pub(crate) program_headers_offset: u64,
    #[pointer_size]
    pub(crate) section_headers_offset: u64,
    pub(crate) flags: u32,
    pub(crate) _elf_header_size: u16,
    pub(crate) program_header_size: u16,
    pub(crate) program_header_count: u16,
    pub(crate) section_header_size: u16,
    pub(crate) section_header_count: u16,
    pub(crate) section_names_table_index: u16,
}

#[derive(RawType)]
pub(crate) struct RawNoteHeader {
    pub(crate) name_size: u32,
    pub(crate) value_size: u32,
    pub(crate) type_: u32,
}
