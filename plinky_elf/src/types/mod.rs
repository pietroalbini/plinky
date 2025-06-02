mod string_table;

pub use self::string_table::ElfStringTable;

use crate::ids::{ElfSectionId, ElfStringId, ElfSymbolId};
use crate::raw::{RawGnuHashHeader, RawGroupFlags, RawHashHeader, RawRel, RawRela, RawSymbol};
use plinky_macros::Bitfield;
use plinky_utils::raw_types::{RawType, RawTypeAsPointerSize};
use plinky_utils::{Bits, Endian};
use std::collections::BTreeMap;
use std::num::NonZeroU64;

#[derive(Debug)]
pub struct ElfObject {
    pub env: ElfEnvironment,
    pub type_: ElfType,
    pub entry: Option<NonZeroU64>,
    pub sections: BTreeMap<ElfSectionId, ElfSection>,
    pub segments: Vec<ElfSegment>,
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

impl From<ElfClass> for Bits {
    fn from(val: ElfClass) -> Self {
        match val {
            ElfClass::Elf32 => Bits::Bits32,
            ElfClass::Elf64 => Bits::Bits64,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElfABI {
    SystemV,
    Gnu,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElfEndian {
    Little,
}

impl From<ElfEndian> for Endian {
    fn from(value: ElfEndian) -> Self {
        match value {
            ElfEndian::Little => Endian::Little,
        }
    }
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
pub struct ElfSection {
    pub name: ElfStringId,
    pub memory_address: u64,
    pub part_of_group: bool,
    pub content: ElfSectionContent,
    pub retain: bool,
}

#[derive(Debug)]
pub enum ElfSectionContent {
    Null,
    Program(ElfProgramSection),
    Uninitialized(ElfUninitializedSection),
    SymbolTable(ElfSymbolTable),
    StringTable(ElfStringTable),
    Rel(ElfRelTable),
    Rela(ElfRelaTable),
    Note(ElfNotesTable),
    Group(ElfGroup),
    Hash(ElfHash),
    GnuHash(ElfGnuHash),
    Dynamic(ElfDynamic),
    Unknown(ElfUnknownSection),
}

impl ElfSectionContent {
    pub fn content_size(&self, bits: ElfClass) -> usize {
        match self {
            ElfSectionContent::Null => 0,
            ElfSectionContent::Program(p) => p.raw.len(),
            ElfSectionContent::Uninitialized(u) => u.len as usize,
            ElfSectionContent::SymbolTable(s) => RawSymbol::size(bits) * s.symbols.len(),
            ElfSectionContent::StringTable(s) => s.len(),
            ElfSectionContent::Rel(r) => RawRel::size(bits) * r.relocations.len(),
            ElfSectionContent::Rela(r) => RawRela::size(bits) * r.relocations.len(),
            ElfSectionContent::Group(g) => {
                RawGroupFlags::size(bits) + u32::size(bits) * g.sections.len()
            }
            ElfSectionContent::Hash(h) => {
                RawHashHeader::size(bits)
                    + u32::size(bits) * h.buckets.len()
                    + u32::size(bits) * h.chain.len()
            }
            ElfSectionContent::GnuHash(h) => {
                let bloom_bits = <u64 as RawTypeAsPointerSize>::size(bits);
                RawGnuHashHeader::size(bits)
                    + bloom_bits * h.bloom.len()
                    + u32::size(bits) * h.buckets.len()
                    + u32::size(bits) * h.chain.len()
            }
            ElfSectionContent::Dynamic(d) => {
                let size = <u64 as RawTypeAsPointerSize>::size(bits) * 2;
                d.directives.len() * size
            }
            ElfSectionContent::Note(_) => unimplemented!(),
            ElfSectionContent::Unknown(_) => unimplemented!(),
        }
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub enum ElfDeduplication {
    Disabled,
    ZeroTerminatedStrings,
    FixedSizeChunks { size: NonZeroU64 },
}

#[derive(Debug)]
pub struct ElfProgramSection {
    pub perms: ElfPermissions,
    pub deduplication: ElfDeduplication,
    pub raw: Vec<u8>,
}

#[derive(Debug)]
pub struct ElfUninitializedSection {
    pub perms: ElfPermissions,
    pub len: u64,
}

#[derive(Debug)]
pub struct ElfNotesTable {
    pub notes: Vec<ElfNote>,
}

#[derive(Debug, Clone)]
pub enum ElfNote {
    GnuProperties(Vec<ElfGnuProperty>),
    Unknown(ElfUnknownNote),
}

impl ElfNote {
    pub fn name(&self) -> &str {
        match self {
            ElfNote::GnuProperties(_) => "GNU",
            ElfNote::Unknown(unknown) => &unknown.name,
        }
    }

    pub fn type_(&self) -> u32 {
        match self {
            ElfNote::GnuProperties(_) => 5,
            ElfNote::Unknown(unknown) => unknown.type_,
        }
    }

    pub fn value_len(&self, class: ElfClass) -> usize {
        match self {
            ElfNote::GnuProperties(properties) => properties
                .iter()
                .map(|p| {
                    let mut len = u32::size(class) * 2 + p.value_len(class);
                    let align_to = match class {
                        ElfClass::Elf32 => 4,
                        ElfClass::Elf64 => 8,
                    };
                    if len % align_to != 0 {
                        len += align_to - len % align_to;
                    }
                    len
                })
                .sum(),
            ElfNote::Unknown(unknown) => unknown.value.len(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum ElfGnuProperty {
    X86Features2Used(ElfX86Features2),
    X86IsaUsed(ElfX86Isa),
    Unknown(ElfUnknownGnuProperty),
}

impl ElfGnuProperty {
    fn value_len(&self, class: ElfClass) -> usize {
        match self {
            ElfGnuProperty::X86Features2Used(_) => ElfX86Features2::size(class),
            ElfGnuProperty::X86IsaUsed(_) => ElfX86Isa::size(class),
            ElfGnuProperty::Unknown(unknown) => unknown.data.len(),
        }
    }
}

#[derive(Debug, Bitfield, Clone, Copy)]
#[bitfield_repr(u32)]
#[bitfield_display_comma_separated]
// Defined as GNU_PROPERTY_X86_ISA_1_$name
pub struct ElfX86Isa {
    pub baseline: bool,
    pub v2: bool,
    pub v3: bool,
    pub v4: bool,
}

#[derive(Debug, Bitfield, Clone, Copy)]
#[bitfield_repr(u32)]
#[bitfield_display_comma_separated]
// Defined as GNU_PROPERTY_X86_FEATURE_2_$name
pub struct ElfX86Features2 {
    pub x86: bool,
    pub x87: bool,
    pub mmx: bool,
    pub xmm: bool,
    pub ymm: bool,
    pub zmm: bool,
    pub fxsr: bool,
    pub xsave: bool,
    pub xsaveopt: bool,
    pub xsavec: bool,
    pub tmm: bool,
    pub mask: bool,
}

#[derive(Debug, Clone)]
pub struct ElfUnknownGnuProperty {
    pub type_: u32,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct ElfUnknownNote {
    pub name: String,
    pub value: Vec<u8>,
    pub type_: u32,
}

#[derive(Debug)]
pub struct ElfUnknownSection {
    pub id: u32,
    pub raw: Vec<u8>,
}

#[derive(Debug)]
pub struct ElfSymbolTable {
    pub dynsym: bool,
    pub symbols: BTreeMap<ElfSymbolId, ElfSymbol>,
}

#[derive(Debug)]
pub struct ElfSymbol {
    pub name: ElfStringId,
    pub binding: ElfSymbolBinding,
    pub type_: ElfSymbolType,
    pub visibility: ElfSymbolVisibility,
    pub definition: ElfSymbolDefinition,
    pub value: u64,
    pub size: u64,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ElfSymbolBinding {
    Local,
    Global,
    Weak,
    Unknown(u8),
}

impl std::fmt::Display for ElfSymbolBinding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            ElfSymbolBinding::Local => "Local",
            ElfSymbolBinding::Global => "Global",
            ElfSymbolBinding::Weak => "Weak",
            ElfSymbolBinding::Unknown(unknown) => return write!(f, "<unknown: {unknown:#x}>"),
        })
    }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElfSymbolVisibility {
    Default,
    Hidden,
    Protected,
    Exported,
    Singleton,
    Eliminate,
}

impl std::fmt::Display for ElfSymbolVisibility {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            ElfSymbolVisibility::Default => "Default",
            ElfSymbolVisibility::Hidden => "Hidden",
            ElfSymbolVisibility::Protected => "Protected",
            ElfSymbolVisibility::Exported => "Exported",
            ElfSymbolVisibility::Singleton => "Singleton",
            ElfSymbolVisibility::Eliminate => "Eliminate",
        })
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum ElfSymbolDefinition {
    Undefined,
    Absolute,
    Common,
    Section(ElfSectionId),
}

#[derive(Debug)]
pub struct ElfRelTable {
    pub symbol_table: ElfSectionId,
    pub applies_to_section: ElfSectionId,
    pub relocations: Vec<ElfRel>,
}

#[derive(Debug)]
pub struct ElfRelaTable {
    pub symbol_table: ElfSectionId,
    pub applies_to_section: ElfSectionId,
    pub relocations: Vec<ElfRela>,
}

#[derive(Debug)]
pub struct ElfRel {
    pub offset: u64,
    pub symbol: ElfSymbolId,
    pub relocation_type: ElfRelocationType,
}

#[derive(Debug)]
pub struct ElfRela {
    pub offset: u64,
    pub symbol: ElfSymbolId,
    pub relocation_type: ElfRelocationType,
    pub addend: i64,
}

#[derive(Debug, Clone, Copy)]
#[allow(non_camel_case_types)]
pub enum ElfRelocationType {
    // x86
    X86_None,
    X86_32,
    X86_PC32,
    X86_GOT32,
    X86_PLT32,
    X86_COPY,
    X86_GlobDat,
    X86_JumpSlot,
    X86_Relative,
    X86_GOTOff,
    X86_GOTPC,
    X86_GOT32X,
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
    X86_64_IRelative64,
    X86_64_GOTPCRelX,
    X86_64_Rex_GOTPCRelX,
    X86_64_Code_4_GOTPCRelX,
    X86_64_Code_4_GOTPCOff,
    X86_64_Code_4_GOTPC32_TLSDesc,
    X86_64_Code_5_GOTPCRelX,
    X86_64_Code_5_GOTPCOff,
    X86_64_Code_5_GOTPC32_TLSDesc,
    X86_64_Code_6_GOTPCRelX,
    X86_64_Code_6_GOTPCOff,
    X86_64_Code_6_GOTPC32_TLSDesc,
    // Other:
    Unknown(u32),
}

#[derive(Debug)]
pub struct ElfGroup {
    pub symbol_table: ElfSectionId,
    pub signature: ElfSymbolId,
    pub sections: Vec<ElfSectionId>,
    pub comdat: bool,
}

#[derive(Debug)]
pub struct ElfHash {
    pub symbol_table: ElfSectionId,
    pub buckets: Vec<u32>,
    pub chain: Vec<u32>,
}

#[derive(Debug)]
pub struct ElfGnuHash {
    pub symbol_table: ElfSectionId,
    pub bloom_shift: u32,
    pub symbols_offset: u32,
    pub bloom: Vec<u64>,
    pub buckets: Vec<u32>,
    pub chain: Vec<u32>,
}

#[derive(Debug)]
pub struct ElfDynamic {
    pub string_table: ElfSectionId,
    pub directives: Vec<ElfDynamicDirective>,
}

#[derive(Debug)]
pub enum ElfDynamicDirective {
    Null,
    Needed { string_table_offset: u64 },
    PLTRelocationsSize { bytes: u64 },
    PLTGOT { address: u64 },
    Hash { address: u64 },
    GnuHash { address: u64 },
    StringTable { address: u64 },
    SymbolTable { address: u64 },
    Rela { address: u64 },
    RelaSize { bytes: u64 },
    RelaEntrySize { bytes: u64 },
    StringTableSize { bytes: u64 },
    SymbolTableEntrySize { bytes: u64 },
    InitFunction { address: u64 },
    FiniFunction { address: u64 },
    SharedObjectName { string_table_offset: u64 },
    RuntimePath { string_table_offset: u64 },
    Symbolic,
    Rel { address: u64 },
    RelSize { bytes: u64 },
    RelEntrySize { bytes: u64 },
    PTLRelocationsMode { mode: ElfPLTRelocationsMode },
    Debug { address: u64 },
    RelocationsWillModifyText,
    JumpRel { address: u64 },
    BindNow,
    Flags(ElfDynamicFlags),
    Flags1(ElfDynamicFlags1),
    Unknown { tag: u64, value: u64 },
}

#[derive(Debug, Bitfield, Clone)]
#[bitfield_repr(u64)]
#[bitfield_display_comma_separated]
pub struct ElfDynamicFlags {
    #[bit(3)]
    pub bind_now: bool,
}

#[derive(Debug, Bitfield, Clone)]
#[bitfield_repr(u64)]
#[bitfield_display_comma_separated]
pub struct ElfDynamicFlags1 {
    #[bit(0)]
    pub now: bool,
    #[bit(27)]
    pub pie: bool,
}

#[derive(Debug)]
pub enum ElfPLTRelocationsMode {
    Rel,
    Rela,
    Unknown(u64),
}

#[derive(Debug, Clone)]
pub struct ElfSegment {
    pub type_: ElfSegmentType,
    pub perms: ElfPermissions,
    pub align: u64,
    pub file_offset: u64,
    pub file_size: u64,
    pub virtual_address: u64,
    pub memory_size: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Eq, Ord)]
pub enum ElfSegmentType {
    ProgramHeaderTable,
    Interpreter,
    Load,
    Dynamic,
    Note,
    GnuStack,
    GnuRelro,
    GnuProperty,
    Null,
    Unknown(u32),
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ElfPermissions {
    pub read: bool,
    pub write: bool,
    pub execute: bool,
}

impl ElfPermissions {
    pub const EMPTY: Self = Self { read: false, write: false, execute: false };

    pub const R: Self = Self { read: true, write: false, execute: false };
    pub const W: Self = Self { read: false, write: true, execute: false };
    pub const X: Self = Self { read: false, write: false, execute: true };

    pub const RW: Self = Self { read: true, write: true, execute: false };
    pub const RX: Self = Self { read: true, write: false, execute: true };
    pub const WX: Self = Self { read: false, write: true, execute: true };

    pub const RWX: Self = Self { read: true, write: true, execute: true };
}

impl std::fmt::Debug for ElfPermissions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ElfPermissions({self})")
    }
}

impl std::fmt::Display for ElfPermissions {
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
        f.write_str(&content)
    }
}
