use plinky_macros::{Bitfield, RawType};
use plinky_utils::raw_types::RawPadding;

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
    pub(crate) flags: RawHeaderFlags,
    pub(crate) elf_header_size: u16,
    pub(crate) program_header_size: u16,
    pub(crate) program_header_count: u16,
    pub(crate) section_header_size: u16,
    pub(crate) section_header_count: u16,
    pub(crate) section_names_table_index: u16,
}

#[derive(Bitfield)]
#[bitfield_repr(u32)]
pub(crate) struct RawHeaderFlags;

#[derive(RawType)]
pub(crate) struct RawSectionHeader {
    pub(crate) name_offset: u32,
    pub(crate) type_: u32,
    #[pointer_size]
    pub(crate) flags: RawSectionHeaderFlags,
    #[pointer_size]
    pub(crate) memory_address: u64,
    #[pointer_size]
    pub(crate) offset: u64,
    #[pointer_size]
    pub(crate) size: u64,
    pub(crate) link: u32,
    pub(crate) info: u32,
    #[pointer_size]
    pub(crate) addr_align: u64,
    #[pointer_size]
    pub(crate) entries_size: u64,
}

#[derive(Bitfield)]
#[bitfield_repr(u64)]
pub(crate) struct RawSectionHeaderFlags {
    pub(crate) write: bool,
    pub(crate) alloc: bool,
    pub(crate) exec: bool,
    #[bit(4)]
    pub(crate) merge: bool,
    #[bit(5)]
    pub(crate) strings: bool,
    #[bit(6)]
    pub(crate) info_link: bool,
    #[bit(9)]
    pub(crate) group: bool,
}

#[derive(RawType)]
pub(crate) struct RawSymbol {
    pub(crate) name_offset: u32,
    pub(crate) info: u8,
    pub(crate) reserved: RawPadding<1>,
    pub(crate) definition: u16,
    #[pointer_size]
    #[placed_on_elf32_after = "name_offset"]
    pub(crate) value: u64,
    #[pointer_size]
    #[placed_on_elf32_after = "value"]
    pub(crate) size: u64,
}

#[derive(RawType)]
pub(crate) struct RawNoteHeader {
    pub(crate) name_size: u32,
    pub(crate) value_size: u32,
    pub(crate) type_: u32,
}

#[derive(RawType)]
pub(crate) struct RawProgramHeader {
    pub(crate) type_: u32,
    #[pointer_size]
    pub(crate) file_offset: u64,
    #[pointer_size]
    pub(crate) virtual_address: u64,
    #[pointer_size]
    pub(crate) reserved: u64,
    #[pointer_size]
    pub(crate) file_size: u64,
    #[pointer_size]
    pub(crate) memory_size: u64,
    #[placed_on_elf64_after = "type_"]
    pub(crate) flags: RawProgramHeaderFlags,
    #[pointer_size]
    pub(crate) align: u64,
}

#[derive(Bitfield)]
#[bitfield_repr(u32)]
pub(crate) struct RawProgramHeaderFlags {
    pub(crate) execute: bool,
    pub(crate) write: bool,
    pub(crate) read: bool,
}

#[derive(RawType)]
pub(crate) struct RawRel {
    #[pointer_size]
    pub(crate) offset: u64,
    #[pointer_size]
    pub(crate) info: u64,
}

#[derive(RawType)]
pub(crate) struct RawRela {
    #[pointer_size]
    pub(crate) offset: u64,
    #[pointer_size]
    pub(crate) info: u64,
    #[pointer_size]
    pub(crate) addend: i64,
}

#[derive(Bitfield)]
#[bitfield_repr(u32)]
pub(crate) struct RawGroupFlags {
    pub(crate) comdat: bool,
}
