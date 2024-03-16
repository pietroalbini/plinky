use crate::cli::DebugPrint;
use crate::linker::LinkerCallbacks;
use crate::repr::object::{DataSectionPart, Object, SectionContent, SectionLayout};
use plinky_diagnostics::widgets::{Table, Text, Widget};
use plinky_diagnostics::{Diagnostic, DiagnosticKind};
use plinky_elf::ids::serial::SerialIds;
use plinky_elf::{ElfObject, ElfSymbol, ElfSymbolBinding, ElfSymbolDefinition, ElfSymbolType};
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
    diagnostic = diagnostic.add(render_symbols(object, object.symbols.iter_local()));
    diagnostic = diagnostic.add(render_symbols(
        object,
        object.symbols.iter_global().map(|(_, symbol)| match symbol {
            crate::repr::symbols::GlobalSymbol::Strong(symbol) => symbol,
            crate::repr::symbols::GlobalSymbol::Undefined => todo!(),
        }),
    ));
    diagnostic = diagnostic.add(Text::new(format!("symbols: {:#?}", object.symbols)));

    eprintln!("{diagnostic}\n");
}

fn render_symbols<'a, T>(
    object: &Object<T>,
    symbols: impl Iterator<Item = &'a ElfSymbol<SerialIds>>,
) -> Table {
    let mut table = Table::new();
    table.add_row(["Name", "Binding", "Type", "Value"]);
    for symbol in symbols {
        let name = match (&symbol.type_, &symbol.definition) {
            (ElfSymbolType::Section, ElfSymbolDefinition::Section(id)) => object
                .section_ids_to_names
                .get(id)
                .map(|s| s.resolve().to_string())
                .unwrap_or_else(|| "<unknown>".to_string()),
            _ => object.strings.get(symbol.name).unwrap_or("<unknown>").to_string(),
        };
        let binding = match symbol.binding {
            ElfSymbolBinding::Local => "local",
            ElfSymbolBinding::Global => "global",
            ElfSymbolBinding::Weak => "weak",
            ElfSymbolBinding::Unknown(_) => "unknown",
        };
        let type_ = match symbol.type_ {
            ElfSymbolType::NoType => "no type",
            ElfSymbolType::Object => "object",
            ElfSymbolType::Function => "function",
            ElfSymbolType::Section => "section",
            ElfSymbolType::File => "file",
            ElfSymbolType::Unknown(_) => "unknown",
        };
        let value = match (&symbol.definition, &symbol.type_) {
            (ElfSymbolDefinition::Undefined, _) => "<undefined>".to_string(),
            (ElfSymbolDefinition::Absolute, ElfSymbolType::File) => "-".to_string(),
            (ElfSymbolDefinition::Absolute, _) => format!("{:#x}", symbol.value),
            (ElfSymbolDefinition::Common, _) => todo!(),
            (ElfSymbolDefinition::Section(_), ElfSymbolType::Section) => "-".to_string(),
            (ElfSymbolDefinition::Section(id), _) => {
                let section_name = object
                    .section_ids_to_names
                    .get(id)
                    .map(|name| name.resolve().to_string())
                    .unwrap_or_else(|| "<unknown section>".into());
                format!("{section_name} + {:#x}", symbol.value)
            }
        };
        table.add_row([&name, binding, type_, &value]);
    }
    table
}

fn render<T: Widget + 'static>(message: &str, widget: T) {
    let diagnostic = Diagnostic::new(DiagnosticKind::DebugPrint, message).add(widget);
    eprintln!("{diagnostic}\n");
}
