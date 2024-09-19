use crate::passes::build_elf::ids::{BuiltElfIds, BuiltElfSectionId, BuiltElfSymbolId};
use crate::repr::relocations::{Relocation, RelocationType};
use crate::utils::address_resolver::{AddressResolutionError, AddressResolver};
use plinky_elf::ids::serial::SymbolId;
use plinky_elf::{
    ElfClass, ElfRelocation, ElfRelocationType, ElfRelocationsTable, ElfSectionContent,
};
use plinky_macros::{Display, Error};
use plinky_utils::ints::ExtractNumber;
use std::collections::BTreeMap;

pub(super) fn create_rela<'a>(
    relocations: impl Iterator<Item = &'a Relocation>,
    class: ElfClass,
    applies_to_section: BuiltElfSectionId,
    symbol_table: BuiltElfSectionId,
    symbol_conversion: &BTreeMap<SymbolId, BuiltElfSymbolId>,
    resolver: &AddressResolver<'_>,
) -> Result<ElfSectionContent<BuiltElfIds>, RelaCreationError> {
    let mut elf_relocations = Vec::new();
    for relocation in relocations {
        elf_relocations.push(ElfRelocation {
            offset: resolver.address(relocation.section, relocation.offset)?.1.extract(),
            symbol: *symbol_conversion.get(&relocation.symbol).unwrap(),
            relocation_type: convert_relocation_type(class, relocation.type_),
            addend: relocation.addend.map(|off| off.extract()),
        });
    }

    Ok(ElfSectionContent::RelocationsTable(ElfRelocationsTable {
        symbol_table,
        applies_to_section,
        relocations: elf_relocations,
    }))
}

fn convert_relocation_type(class: ElfClass, type_: RelocationType) -> ElfRelocationType {
    macro_rules! unsupported {
        () => {{
            panic!("converting {type_:?} for {class:?} is not supported");
        }};
    }

    match (&class, &type_) {
        (ElfClass::Elf32, RelocationType::Absolute32) => ElfRelocationType::X86_32,
        (ElfClass::Elf32, RelocationType::AbsoluteSigned32) => unsupported!(),
        (ElfClass::Elf32, RelocationType::Relative32) => ElfRelocationType::X86_PC32,
        (ElfClass::Elf32, RelocationType::PLT32) => unsupported!(),
        (ElfClass::Elf32, RelocationType::GOTRelative32) => unsupported!(),
        (ElfClass::Elf32, RelocationType::GOTIndex32) => ElfRelocationType::X86_GOT32,
        (ElfClass::Elf32, RelocationType::GOTLocationRelative32) => ElfRelocationType::X86_GOTPC,
        (ElfClass::Elf32, RelocationType::OffsetFromGOT32) => ElfRelocationType::X86_GOTOff,
        (ElfClass::Elf32, RelocationType::FillGotSlot) => ElfRelocationType::X86_GlobDat,
        (ElfClass::Elf32, RelocationType::FillGotPltSlot) => ElfRelocationType::X86_JumpSlot,

        (ElfClass::Elf64, RelocationType::Absolute32) => ElfRelocationType::X86_64_32,
        (ElfClass::Elf64, RelocationType::AbsoluteSigned32) => ElfRelocationType::X86_64_32S,
        (ElfClass::Elf64, RelocationType::Relative32) => ElfRelocationType::X86_64_PC32,
        (ElfClass::Elf64, RelocationType::PLT32) => ElfRelocationType::X86_64_PLT32,
        (ElfClass::Elf64, RelocationType::GOTRelative32) => ElfRelocationType::X86_64_GOTPCRel,
        (ElfClass::Elf64, RelocationType::GOTIndex32) => unsupported!(),
        (ElfClass::Elf64, RelocationType::GOTLocationRelative32) => unsupported!(),
        (ElfClass::Elf64, RelocationType::OffsetFromGOT32) => unsupported!(),
        (ElfClass::Elf64, RelocationType::FillGotSlot) => ElfRelocationType::X86_64_GlobDat,
        (ElfClass::Elf64, RelocationType::FillGotPltSlot) => ElfRelocationType::X86_64_JumpSlot,
    }
}

#[derive(Debug, Error, Display)]
pub(crate) enum RelaCreationError {
    #[display("failed to resolve address of section")]
    AddressResolution(#[from] AddressResolutionError),
}
