use plinky_macros::{Bitfield, RawType};
use plinky_utils::raw_types::RawPadding;

#[derive(RawType)]
pub struct RawIdentification {
    pub magic: [u8; 4],
    pub class: u8,
    pub endian: u8,
    pub version: u8,
    pub abi: u8,
    pub abi_version: u8,
    pub padding: RawPadding<7>,
}

#[derive(RawType)]
pub struct RawHeader {
    pub type_: u16,
    pub machine: u16,
    pub version: u32,
    #[pointer_size]
    pub entry: u64,
    #[pointer_size]
    pub program_headers_offset: u64,
    #[pointer_size]
    pub section_headers_offset: u64,
    pub flags: RawHeaderFlags,
    pub elf_header_size: u16,
    pub program_header_size: u16,
    pub program_header_count: u16,
    pub section_header_size: u16,
    pub section_header_count: u16,
    pub section_names_table_index: u16,
}

#[derive(Bitfield)]
#[bitfield_repr(u32)]
pub struct RawHeaderFlags;

#[derive(RawType)]
pub struct RawSectionHeader {
    pub name_offset: u32,
    pub type_: u32,
    #[pointer_size]
    pub flags: RawSectionHeaderFlags,
    #[pointer_size]
    pub memory_address: u64,
    #[pointer_size]
    pub offset: u64,
    #[pointer_size]
    pub size: u64,
    pub link: u32,
    pub info: u32,
    #[pointer_size]
    pub addr_align: u64,
    #[pointer_size]
    pub entries_size: u64,
}

#[derive(Bitfield)]
#[bitfield_repr(u64)]
pub struct RawSectionHeaderFlags {
    pub write: bool,
    pub alloc: bool,
    pub exec: bool,
    #[bit(4)]
    pub merge: bool,
    #[bit(5)]
    pub strings: bool,
    #[bit(6)]
    pub info_link: bool,
    #[bit(9)]
    pub group: bool,
    /// `SHF_GNU_RETAIN`
    #[bit(21)]
    pub gnu_retain: bool,
}

#[derive(RawType)]
pub struct RawSymbol {
    pub name_offset: u32,
    pub info: u8,
    pub other: u8,
    pub definition: u16,
    #[pointer_size]
    #[placed_on_elf32_after = "name_offset"]
    pub value: u64,
    #[pointer_size]
    #[placed_on_elf32_after = "value"]
    pub size: u64,
}

#[derive(RawType)]
pub struct RawNoteHeader {
    pub name_size: u32,
    pub value_size: u32,
    pub type_: u32,
}

#[derive(RawType)]
pub struct RawProgramHeader {
    pub type_: u32,
    #[pointer_size]
    pub file_offset: u64,
    #[pointer_size]
    pub virtual_address: u64,
    #[pointer_size]
    pub reserved: u64,
    #[pointer_size]
    pub file_size: u64,
    #[pointer_size]
    pub memory_size: u64,
    #[placed_on_elf64_after = "type_"]
    pub flags: RawProgramHeaderFlags,
    #[pointer_size]
    pub align: u64,
}

#[derive(Bitfield)]
#[bitfield_repr(u32)]
pub struct RawProgramHeaderFlags {
    pub execute: bool,
    pub write: bool,
    pub read: bool,
}

#[derive(RawType)]
pub struct RawRel {
    #[pointer_size]
    pub offset: u64,
    #[pointer_size]
    pub info: u64,
}

#[derive(RawType)]
pub struct RawRela {
    #[pointer_size]
    pub offset: u64,
    #[pointer_size]
    pub info: u64,
    #[pointer_size]
    pub addend: i64,
}

#[derive(RawType)]
pub struct RawHashHeader {
    pub bucket_count: u32,
    pub chain_count: u32,
}

#[derive(RawType)]
pub struct RawGnuHashHeader {
    pub buckets_count: u32,
    pub symbols_offset: u32,
    pub bloom_count: u32,
    pub bloom_shift: u32,
}

#[derive(Bitfield)]
#[bitfield_repr(u32)]
pub struct RawGroupFlags {
    pub comdat: bool,
}
