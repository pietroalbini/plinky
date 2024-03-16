use crate::cli::DebugPrint;
use crate::linker::LinkerCallbacks;
use crate::repr::object::{DataSectionPart, Object, SectionContent, SectionLayout};
use crate::repr::symbols::{Symbol, SymbolValue, SymbolVisibility};
use plinky_diagnostics::widgets::{Table, Text, Widget};
use plinky_diagnostics::{Diagnostic, DiagnosticKind};
use plinky_elf::ids::serial::SerialIds;
use plinky_elf::ElfObject;
use std::collections::BTreeSet;
use std::fmt::Debug;

pub(crate) struct DebugCallbacks {
    pub(crate) print: BTreeSet<DebugPrint>,
}

impl LinkerCallbacks for DebugCallbacks {
    fn on_inputs_loaded(&self, object: &Object<()>) {
        if self.print.contains(&DebugPrint::LoadedObject) {
            render_object("loaded object", object);
        }
    }

    fn on_layout_calculated(&self, object: &Object<SectionLayout>) {
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
    }

    fn on_relocations_applied(&self, object: &Object<SectionLayout>) {
        if self.print.contains(&DebugPrint::RelocatedObject) {
            render_object("object after relocations are applied", object);
        }
    }

    fn on_elf_built(&self, elf: &ElfObject<SerialIds>) {
        if self.print.contains(&DebugPrint::FinalElf) {
            render("built elf", Text::new(format!("{elf:#x?}")));
        }
    }
}

fn render_object<T: Debug>(message: &str, object: &Object<T>) {
    let mut diagnostic = Diagnostic::new(DiagnosticKind::DebugPrint, message);

    diagnostic = diagnostic.add(Text::new(format!("env: {:#?}", object.env)));
    diagnostic = diagnostic.add(Text::new(format!("sections: {:#?}", object.sections)));
    diagnostic = diagnostic.add(render_symbols(object, object.symbols.iter()));

    eprintln!("{diagnostic}\n");
}

fn render_symbols<'a, T>(object: &Object<T>, symbols: impl Iterator<Item = &'a Symbol>) -> Table {
    let mut symbols = symbols.collect::<Vec<_>>();
    symbols.sort_by_key(|symbol| symbol.name);

    let mut table = Table::new();
    table.add_row(["Name", "Visibility", "Value"]);
    for symbol in symbols {
        let visibility = match symbol.visibility {
            SymbolVisibility::Local => "local",
            SymbolVisibility::Global { weak: true } => "global (weak)",
            SymbolVisibility::Global { weak: false } => "global",
        };
        let value = match symbol.value {
            SymbolValue::Absolute { value } => format!("{value:#x}"),
            SymbolValue::SectionRelative { section, offset } => {
                let section_name = object
                    .section_ids_to_names
                    .get(&section)
                    .map(|name| name.resolve().to_string())
                    .unwrap_or_else(|| "<unknown section>".into());
                format!("{section_name} + {offset:#x}")
            }
            SymbolValue::Undefined => "<undefined>".into(),
        };
        table.add_row([&symbol.name.to_string(), visibility, &value]);
    }
    table
}

fn render<T: Widget + 'static>(message: &str, widget: T) {
    let diagnostic = Diagnostic::new(DiagnosticKind::DebugPrint, message).add(widget);
    eprintln!("{diagnostic}\n");
}
