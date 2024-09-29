use crate::repr::object::Object;
use crate::repr::relocations::Relocation;
use crate::repr::sections::{DataSection, SectionContent};
use crate::repr::symbols::Symbol;
use plinky_elf::ids::serial::{SectionId, SerialIds, SymbolId};
use plinky_elf::{ElfMachine, ElfPermissions};
use plinky_utils::ints::Offset;
use std::collections::BTreeMap;

pub(crate) fn run(ids: &mut SerialIds, object: &mut Object) {
    let Some(got_plt) = &object.got_plt else { return };
    let got_plt_section = got_plt.id;

    // The .got.plt might not have any entries within it if it was generated just to fulfill the
    // need to have a _GLOBAL_OFFSET_TABLE_ symbol. In that case, there is no need to have a PLT.
    if got_plt.offsets.is_empty() {
        return;
    }

    let plt_section = ids.allocate_section_id();
    let plt_symbol = ids.allocate_symbol_id();
    object.symbols.add_symbol(Symbol::new_for_section(plt_symbol, plt_section)).unwrap();

    let output = match object.env.machine {
        ElfMachine::X86 => crate::arch::x86::generate_plt(got_plt, plt_symbol),
        ElfMachine::X86_64 => crate::arch::x86_64::generate_plt(got_plt, plt_symbol),
    };

    let mut data = DataSection::new(ElfPermissions::RX, &output.content);
    data.relocations.extend(output.relocations.into_iter());

    object.sections.builder(".plt", data).create_with_id(plt_section);

    object.plt = Some(Plt { section: plt_section, offsets: output.offsets });

    // In some cases, generating the PLT requires adding additional relocations to the .got.plt.
    // Check the arch modules for an explaination on why they need this.
    match &mut object.sections.get_mut(got_plt_section).unwrap().content {
        SectionContent::Data(data_section) => {
            data_section.relocations.extend(output.extra_got_plt_relocations.into_iter());
        }
        _ => panic!(".got.plt must be a data section"),
    }
}

#[derive(Debug)]
pub(crate) struct Plt {
    pub(crate) section: SectionId,
    pub(crate) offsets: BTreeMap<SymbolId, Offset>,
}

pub(crate) struct GeneratePltArchOutput {
    pub(crate) content: Vec<u8>,
    pub(crate) relocations: Vec<Relocation>,
    pub(crate) extra_got_plt_relocations: Vec<Relocation>,
    pub(crate) offsets: BTreeMap<SymbolId, Offset>,
}
