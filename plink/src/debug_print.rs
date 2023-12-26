use crate::cli::DebugPrint;
use crate::linker::{CallbackOutcome, LinkerCallbacks};
use crate::repr::object::{DataSectionPart, Object, SectionContent, SectionLayout};
use plink_diagnostics::widgets::Table;
use plink_elf::ids::serial::SerialIds;
use plink_elf::ElfObject;

pub(crate) struct DebugCallbacks {
    pub(crate) print: Option<DebugPrint>,
}

impl LinkerCallbacks for DebugCallbacks {
    fn on_inputs_loaded(&self, object: &Object<()>) -> CallbackOutcome {
        if let Some(DebugPrint::LoadedObject) = self.print {
            println!("{object:#x?}");
            CallbackOutcome::Stop
        } else {
            CallbackOutcome::Continue
        }
    }

    fn on_layout_calculated(&self, object: &Object<SectionLayout>) -> CallbackOutcome {
        if let Some(DebugPrint::Layout) = self.print {
            let mut table = Table::new();
            table.add_row(["Internal ID", "Section name", "Source object", "Memory address"]);

            for (name, section) in &object.sections {
                match &section.content {
                    SectionContent::Data(data) => {
                        for (id, part) in &data.parts {
                            let (source, address) = match part {
                                DataSectionPart::Real(real) => {
                                    (&real.source, format!("{:#x}", real.layout.address))
                                }
                                DataSectionPart::DeduplicationFacade(facade) => {
                                    (&facade.source, "N/A (deduplication facade)".into())
                                }
                            };
                            table.add_row([
                                format!("{id:?}"),
                                name.to_string(),
                                source.to_string(),
                                address,
                            ]);
                        }
                    }
                    SectionContent::Uninitialized(parts) => {
                        for (id, part) in parts {
                            table.add_row([
                                format!("{id:?}"),
                                name.to_string(),
                                part.source.to_string(),
                                format!("{:#x}", part.layout.address),
                            ]);
                        }
                    }
                }
            }

            println!("Section addresses");
            println!("-----------------");
            println!("{}", table.render());

            CallbackOutcome::Stop
        } else {
            CallbackOutcome::Continue
        }
    }

    fn on_relocations_applied(&self, object: &Object<SectionLayout>) -> CallbackOutcome {
        if let Some(DebugPrint::RelocatedObject) = self.print {
            println!("{object:#x?}");
            CallbackOutcome::Stop
        } else {
            CallbackOutcome::Continue
        }
    }

    fn on_elf_built(&self, elf: &ElfObject<SerialIds>) -> CallbackOutcome {
        if let Some(DebugPrint::FinalElf) = self.print {
            println!("{elf:#x?}");
            CallbackOutcome::Stop
        } else {
            CallbackOutcome::Continue
        }
    }
}
