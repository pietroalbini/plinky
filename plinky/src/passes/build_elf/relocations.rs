use crate::passes::build_elf::symbols::AddSymbolsOutput;
use crate::passes::build_elf::ElfBuilder;
use crate::repr::object::Object;
use crate::repr::relocations::{Relocation, RelocationType};
use crate::utils::ints::ExtractNumber;
use plinky_elf::{ElfClass, ElfRelocation, ElfRelocationType, ElfRelocationsTable};

pub(crate) fn add_rela<'a, F, I>(
    builder: &'a mut ElfBuilder,
    name: &str,
    symtab: &AddSymbolsOutput,
    getter: F,
) where
    F: FnOnce(&'a Object) -> I,
    I: Iterator<Item = &'a Relocation>,
{
    let mut elf_relocations = Vec::new();
    for relocation in getter(&builder.object) {
        elf_relocations.push(ElfRelocation {
            offset: relocation.offset.extract().try_into().unwrap(),
            symbol: *symtab.conversion.get(&relocation.symbol).unwrap(),
            relocation_type: convert_relocation_type(builder.object.env.class, relocation.type_),
            addend: relocation.addend.map(|off| off.extract()),
        });
    }

    builder
        .sections
        .create(
            name,
            plinky_elf::ElfSectionContent::RelocationsTable(ElfRelocationsTable {
                symbol_table: symtab.table,
                applies_to_section: builder.sections.zero_id,
                relocations: elf_relocations,
            }),
        )
        .add(&mut builder.ids);
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
        (ElfClass::Elf32, RelocationType::FillGOTSlot) => ElfRelocationType::X86_GLOB_DAT,

        (ElfClass::Elf64, RelocationType::Absolute32) => ElfRelocationType::X86_64_32,
        (ElfClass::Elf64, RelocationType::AbsoluteSigned32) => ElfRelocationType::X86_64_32S,
        (ElfClass::Elf64, RelocationType::Relative32) => ElfRelocationType::X86_64_PC32,
        (ElfClass::Elf64, RelocationType::PLT32) => ElfRelocationType::X86_64_PLT32,
        (ElfClass::Elf64, RelocationType::GOTRelative32) => ElfRelocationType::X86_64_GOTPCRel,
        (ElfClass::Elf64, RelocationType::GOTIndex32) => unsupported!(),
        (ElfClass::Elf64, RelocationType::GOTLocationRelative32) => unsupported!(),
        (ElfClass::Elf64, RelocationType::OffsetFromGOT32) => unsupported!(),
        (ElfClass::Elf64, RelocationType::FillGOTSlot) => ElfRelocationType::X86_64_GlobDat,
    }
}
