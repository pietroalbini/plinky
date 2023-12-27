use crate::cli::DebugPrint;
use crate::linker::{CallbackOutcome, LinkerCallbacks};
use crate::repr::object::{DataSectionPart, Object, SectionContent, SectionLayout};
use plink_diagnostics::widgets::{Table, Text, Widget};
use plink_diagnostics::{Diagnostic, DiagnosticKind};
use plink_elf::ids::serial::SerialIds;
use plink_elf::ElfObject;
use std::collections::BTreeSet;

pub(crate) struct DebugCallbacks {
    pub(crate) print: BTreeSet<DebugPrint>,
}

impl LinkerCallbacks for DebugCallbacks {
    fn on_inputs_loaded(&self, object: &Object<()>) -> CallbackOutcome {
        if self.print.contains(&DebugPrint::LoadedObject) {
            render("loaded object", Text::new(format!("{object:#x?}")));
        }
        CallbackOutcome::Continue
    }

    fn on_layout_calculated(&self, object: &Object<SectionLayout>) -> CallbackOutcome {
        if self.print.contains(&DebugPrint::Layout) {
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

            render("calculated layout", table);
        }
        CallbackOutcome::Continue
    }

    fn on_relocations_applied(&self, object: &Object<SectionLayout>) -> CallbackOutcome {
        if self.print.contains(&DebugPrint::RelocatedObject) {
            render("object after relocations are applied", Text::new(format!("{object:#x?}")));
        }
        CallbackOutcome::Continue
    }

    fn on_elf_built(&self, elf: &ElfObject<SerialIds>) -> CallbackOutcome {
        if self.print.contains(&DebugPrint::FinalElf) {
            render("built elf", Text::new(format!("{elf:#x?}")));
        }
        CallbackOutcome::Continue
    }
}

fn render<T: Widget + 'static>(message: &str, widget: T) {
    let diagnostic = Diagnostic::new(DiagnosticKind::DebugPrint, message).add(widget);
    eprintln!("{diagnostic}\n");
}
