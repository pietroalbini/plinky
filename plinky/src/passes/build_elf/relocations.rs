use crate::repr::relocations::{Relocation, RelocationAddend, RelocationMode, RelocationType};
use crate::repr::sections::SectionId;
use crate::repr::symbols::SymbolId;
use crate::utils::address_resolver::{AddressResolutionError, AddressResolver};
use plinky_elf::ids::{ElfSectionId, ElfSymbolId};
use plinky_elf::{
    ElfClass, ElfRel, ElfRelTable, ElfRela, ElfRelaTable, ElfRelocationType, ElfSectionContent,
};
use plinky_macros::{Display, Error};
use plinky_utils::ints::ExtractNumber;
use std::collections::BTreeMap;

pub(super) fn create_relocations<'a>(
    mode: RelocationMode,
    section: SectionId,
    relocations: impl Iterator<Item = &'a Relocation>,
    class: ElfClass,
    applies_to_section: ElfSectionId,
    symbol_table: ElfSectionId,
    symbol_conversion: &BTreeMap<SymbolId, ElfSymbolId>,
    resolver: &AddressResolver<'_>,
) -> Result<ElfSectionContent, RelaCreationError> {
    let mut generic = Vec::new();
    for relocation in relocations {
        generic.push(GenericRelocation {
            offset: resolver.address(section, relocation.offset)?.1.extract(),
            symbol: *symbol_conversion.get(&relocation.symbol).unwrap(),
            type_: convert_relocation_type(class, relocation.type_),
            addend: relocation.addend,
        });
    }

    Ok(match mode {
        RelocationMode::Rel => create_rel(symbol_table, applies_to_section, generic),
        RelocationMode::Rela => create_rela(symbol_table, applies_to_section, generic),
    })
}

fn create_rel(
    symbol_table: ElfSectionId,
    applies_to_section: ElfSectionId,
    generic: Vec<GenericRelocation>,
) -> ElfSectionContent {
    ElfSectionContent::Rel(ElfRelTable {
        symbol_table,
        applies_to_section,
        relocations: generic
            .into_iter()
            .map(|generic| {
                let RelocationAddend::Inline = generic.addend else {
                    panic!("addend for rel is not inline");
                };
                ElfRel {
                    offset: generic.offset,
                    symbol: generic.symbol,
                    relocation_type: generic.type_,
                }
            })
            .collect(),
    })
}

fn create_rela(
    symbol_table: ElfSectionId,
    applies_to_section: ElfSectionId,
    generic: Vec<GenericRelocation>,
) -> ElfSectionContent {
    ElfSectionContent::Rela(ElfRelaTable {
        symbol_table,
        applies_to_section,
        relocations: generic
            .into_iter()
            .map(|generic| {
                let RelocationAddend::Explicit(addend) = generic.addend else {
                    panic!("addend for rela is not explicit");
                };
                ElfRela {
                    offset: generic.offset,
                    symbol: generic.symbol,
                    relocation_type: generic.type_,
                    addend: addend.extract(),
                }
            })
            .collect(),
    })
}

struct GenericRelocation {
    offset: u64,
    symbol: ElfSymbolId,
    type_: ElfRelocationType,
    addend: RelocationAddend,
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
