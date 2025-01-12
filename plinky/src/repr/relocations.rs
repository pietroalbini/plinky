use crate::repr::symbols::SymbolId;
use plinky_elf::ids::ElfSymbolId;
use plinky_elf::{ElfEndian, ElfRel, ElfRela, ElfRelocationType};
use plinky_macros::{Display, Error};
use plinky_utils::ints::{ExtractNumber, Offset};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RelocationMode {
    Rel,
    Rela,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RelocationType {
    Absolute32,
    Absolute64,
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
    pub(crate) fn addend_type(&self) -> AddendType {
        match self {
            RelocationType::Absolute64 => AddendType::I64,
            RelocationType::Absolute32
            | RelocationType::AbsoluteSigned32
            | RelocationType::Relative32
            | RelocationType::PLT32
            | RelocationType::GOTRelative32
            | RelocationType::GOTIndex32
            | RelocationType::GOTLocationRelative32
            | RelocationType::OffsetFromGOT32 => AddendType::I32,
            RelocationType::FillGotSlot | RelocationType::FillGotPltSlot => AddendType::None,
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
            ElfRelocationType::X86_64_64 => RelocationType::Absolute64,
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

    pub(crate) fn addend(
        &self,
        endian: ElfEndian,
        data: &[u8],
    ) -> Result<Offset, RelocationAddendError> {
        match self.addend {
            RelocationAddend::Inline => {}
            RelocationAddend::Explicit(addend) => return Ok(addend),
        }

        match (self.type_.addend_type(), endian) {
            (AddendType::None, _) => return Err(RelocationAddendError::NotSupported(self.type_)),
            (AddendType::I32, ElfEndian::Little) => {
                Ok(i32::from_le_bytes(self.addend_bytes(data)?).into())
            }
            (AddendType::I64, ElfEndian::Little) => {
                Ok(i64::from_le_bytes(self.addend_bytes(data)?).into())
            }
        }
    }

    fn addend_bytes<const N: usize>(&self, data: &[u8]) -> Result<[u8; N], RelocationAddendError> {
        let start = self.offset.extract();
        let end = self.offset.extract() + N as i64;
        if start < 0 || (data.len() as i64) < end {
            return Err(RelocationAddendError::OutOfBounds(self.offset));
        }

        Ok((&data[(start as usize)..(end as usize)]).try_into().unwrap())
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

#[derive(Debug)]
pub(crate) enum AddendType {
    None,
    I32,
    I64,
}

#[derive(Debug, Display, Error)]
#[display("unsupported relocation type {elf_type:?}")]
pub(crate) struct UnsupportedRelocationType {
    elf_type: ElfRelocationType,
}

#[derive(Debug, Display, Error)]
pub(crate) enum RelocationAddendError {
    #[display("relocation type {f0:?} does not support addends")]
    NotSupported(RelocationType),
    #[display("addend at offset {f0:?} is out of section bounds")]
    OutOfBounds(Offset),
}
