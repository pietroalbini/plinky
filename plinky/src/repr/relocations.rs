use plinky_elf::ids::serial::{SectionId, SerialIds, SymbolId};
use plinky_elf::{ElfRelocation, ElfRelocationType};
use plinky_macros::{Display, Error};
use plinky_utils::ints::Offset;

#[derive(Debug, Clone, Copy)]
pub(crate) enum RelocationType {
    Absolute32,
    AbsoluteSigned32,
    Relative32,
    PLT32,
    GOTRelative32,
    GOTIndex32,
    GOTLocationRelative32,
    OffsetFromGOT32,
    FillGOTSlot,
}

impl RelocationType {
    pub(crate) fn needs_got_entry(&self) -> bool {
        match self {
            RelocationType::Absolute32 => false,
            RelocationType::AbsoluteSigned32 => false,
            RelocationType::Relative32 => false,
            RelocationType::PLT32 => false,
            RelocationType::GOTRelative32 => true,
            RelocationType::GOTIndex32 => true,
            RelocationType::GOTLocationRelative32 => false,
            RelocationType::OffsetFromGOT32 => false,
            RelocationType::FillGOTSlot => false,
        }
    }

    pub(crate) fn needs_got_table(&self) -> bool {
        match self {
            RelocationType::OffsetFromGOT32 => true,
            _ => self.needs_got_entry(),
        }
    }
}

#[derive(Debug)]
pub(crate) struct Relocation {
    pub(crate) type_: RelocationType,
    pub(crate) symbol: SymbolId,
    pub(crate) section: SectionId,
    pub(crate) offset: Offset,
    pub(crate) addend: Option<Offset>,
}

impl Relocation {
    pub(crate) fn from_elf(
        section: SectionId,
        elf: ElfRelocation<SerialIds>,
    ) -> Result<Self, UnsupportedRelocationType> {
        Ok(Relocation {
            type_: match elf.relocation_type {
                ElfRelocationType::X86_32 => RelocationType::Absolute32,
                ElfRelocationType::X86_PC32 => RelocationType::Relative32,
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
            symbol: elf.symbol,
            section,
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
