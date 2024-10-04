use crate::repr::symbols::SymbolId;
use plinky_elf::ids::ElfSymbolId;
use plinky_elf::{ElfRelocation, ElfRelocationType};
use plinky_macros::{Display, Error};
use plinky_utils::ints::Offset;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RelocationType {
    Absolute32,
    AbsoluteSigned32,
    Relative32,
    PLT32,
    GOTRelative32,
    GOTIndex32,
    GOTLocationRelative32,
    OffsetFromGOT32,
    FillGotSlot,
    FillGotPltSlot,
}

impl RelocationType {
    pub(crate) fn needs_got_entry(&self) -> NeedsGot {
        match self {
            RelocationType::Absolute32 => NeedsGot::None,
            RelocationType::AbsoluteSigned32 => NeedsGot::None,
            RelocationType::Relative32 => NeedsGot::None,
            RelocationType::PLT32 => NeedsGot::GotPlt,
            RelocationType::GOTRelative32 => NeedsGot::Got,
            RelocationType::GOTIndex32 => NeedsGot::Got,
            RelocationType::GOTLocationRelative32 => NeedsGot::None,
            RelocationType::OffsetFromGOT32 => NeedsGot::None,
            RelocationType::FillGotSlot => NeedsGot::None,
            RelocationType::FillGotPltSlot => NeedsGot::None,
        }
    }

    pub(crate) fn needs_got_table(&self) -> NeedsGot {
        match self {
            RelocationType::OffsetFromGOT32 => NeedsGot::Got,
            _ => self.needs_got_entry(),
        }
    }
}

pub(crate) enum NeedsGot {
    None,
    Got,
    GotPlt,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Relocation {
    pub(crate) type_: RelocationType,
    pub(crate) symbol: SymbolId,
    pub(crate) offset: Offset,
    pub(crate) addend: Option<Offset>,
}

impl Relocation {
    pub(crate) fn from_elf(
        elf: ElfRelocation,
        conversion: &BTreeMap<ElfSymbolId, SymbolId>,
    ) -> Result<Self, UnsupportedRelocationType> {
        Ok(Relocation {
            type_: match elf.relocation_type {
                ElfRelocationType::X86_32 => RelocationType::Absolute32,
                ElfRelocationType::X86_PC32 => RelocationType::Relative32,
                ElfRelocationType::X86_PLT32 => RelocationType::PLT32,
                ElfRelocationType::X86_GOTPC => RelocationType::GOTLocationRelative32,
                ElfRelocationType::X86_GOTOff => RelocationType::OffsetFromGOT32,
                ElfRelocationType::X86_GOT32 => RelocationType::GOTIndex32,
                ElfRelocationType::X86_GOT32X => RelocationType::GOTIndex32,

                ElfRelocationType::X86_64_32 => RelocationType::Absolute32,
                ElfRelocationType::X86_64_32S => RelocationType::AbsoluteSigned32,
                ElfRelocationType::X86_64_PC32 => RelocationType::Relative32,
                ElfRelocationType::X86_64_PLT32 => RelocationType::PLT32,
                ElfRelocationType::X86_64_GOTPCRel => RelocationType::GOTRelative32,
                ElfRelocationType::X86_64_GOTPCRelX => RelocationType::GOTRelative32,

                elf_type => return Err(UnsupportedRelocationType { elf_type }),
            },
            symbol: *conversion.get(&elf.symbol).unwrap(),
            offset: (elf.offset as i64).into(),
            addend: elf.addend.map(|a| a.into()),
        })
    }
}

#[derive(Debug, Display, Error)]
#[display("unsupported relocation type {elf_type:?}")]
pub(crate) struct UnsupportedRelocationType {
    elf_type: ElfRelocationType,
}
