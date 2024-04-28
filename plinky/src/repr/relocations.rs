use plinky_elf::ids::serial::{SerialIds, SymbolId};
use plinky_elf::{ElfRelocation, ElfRelocationType};
use plinky_macros::{Display, Error};

#[derive(Debug)]
pub(crate) enum RelocationType {
    Absolute32,
    Relative32,
    PLT32,
}

#[derive(Debug)]
pub(crate) struct Relocation {
    pub(crate) type_: RelocationType,
    pub(crate) symbol: SymbolId,
    pub(crate) offset: u64,
    pub(crate) addend: Option<i64>,
}

impl TryFrom<ElfRelocation<SerialIds>> for Relocation {
    type Error = UnsupportedRelocationType;

    fn try_from(value: ElfRelocation<SerialIds>) -> Result<Self, Self::Error> {
        Ok(Relocation {
            type_: match value.relocation_type {
                ElfRelocationType::X86_32 => RelocationType::Absolute32,
                ElfRelocationType::X86_PC32 => RelocationType::Relative32,

                ElfRelocationType::X86_64_32 => RelocationType::Absolute32,
                // TODO: handle X86_64_32 and X86_64_32S differently
                ElfRelocationType::X86_64_32S => RelocationType::Absolute32,
                ElfRelocationType::X86_64_PC32 => RelocationType::Relative32,
                ElfRelocationType::X86_64_PLT32 => RelocationType::PLT32,

                elf_type => return Err(UnsupportedRelocationType { elf_type }),
            },
            symbol: value.symbol,
            offset: value.offset,
            addend: value.addend,
        })
    }
}

#[derive(Debug, Display, Error)]
#[display("unsupported relocation type {elf_type:?}")]
pub(crate) struct UnsupportedRelocationType {
    elf_type: ElfRelocationType,
}
