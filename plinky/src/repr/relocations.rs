use crate::repr::symbols::SymbolId;
use plinky_elf::ids::ElfSymbolId;
use plinky_elf::{ElfRel, ElfRela, ElfRelocationType};
use plinky_macros::{Display, Error};
use plinky_utils::ints::Offset;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RelocationMode {
    Rel,
    Rela,
}

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
    pub(crate) fn uses_addend(&self) -> bool {
        match self {
            RelocationType::Absolute32 => true,
            RelocationType::AbsoluteSigned32 => true,
            RelocationType::Relative32 => true,
            RelocationType::PLT32 => true,
            RelocationType::GOTRelative32 => true,
            RelocationType::GOTIndex32 => true,
            RelocationType::GOTLocationRelative32 => true,
            RelocationType::OffsetFromGOT32 => true,
            RelocationType::FillGotSlot => false,
            RelocationType::FillGotPltSlot => false,
        }
    }
}

impl TryFrom<ElfRelocationType> for RelocationType {
    type Error = UnsupportedRelocationType;

    fn try_from(value: ElfRelocationType) -> Result<Self, Self::Error> {
        Ok(match value {
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
            ElfRelocationType::X86_64_Rex_GOTPCRelX => RelocationType::GOTRelative32,

            elf_type => return Err(UnsupportedRelocationType { elf_type }),
        })
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Relocation {
    pub(crate) type_: RelocationType,
    pub(crate) symbol: SymbolId,
    pub(crate) offset: Offset,
    pub(crate) addend: RelocationAddend,
}

impl Relocation {
    pub(crate) fn from_elf_rel(
        elf: ElfRel,
        conversion: &BTreeMap<ElfSymbolId, SymbolId>,
    ) -> Result<Self, UnsupportedRelocationType> {
        Ok(Relocation {
            type_: elf.relocation_type.try_into()?,
            symbol: *conversion.get(&elf.symbol).unwrap(),
            offset: (elf.offset as i64).into(),
            addend: RelocationAddend::Inline,
        })
    }

    pub(crate) fn from_elf_rela(
        elf: ElfRela,
        conversion: &BTreeMap<ElfSymbolId, SymbolId>,
    ) -> Result<Self, UnsupportedRelocationType> {
        Ok(Relocation {
            type_: elf.relocation_type.try_into()?,
            symbol: *conversion.get(&elf.symbol).unwrap(),
            offset: (elf.offset as i64).into(),
            addend: RelocationAddend::Explicit(elf.addend.into()),
        })
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub(crate) enum RelocationAddend {
    Inline,
    Explicit(Offset),
}

impl From<Offset> for RelocationAddend {
    fn from(value: Offset) -> Self {
        RelocationAddend::Explicit(value)
    }
}

#[derive(Debug, Display, Error)]
#[display("unsupported relocation type {elf_type:?}")]
pub(crate) struct UnsupportedRelocationType {
    elf_type: ElfRelocationType,
}
