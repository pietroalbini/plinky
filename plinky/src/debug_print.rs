use crate::cli::DebugPrint;
use crate::linker::LinkerCallbacks;
use crate::repr::object::{
    DataSectionPart, DataSectionPartReal, DeduplicationFacade, Object, Section, SectionContent,
    SectionLayout, UninitializedSectionPart,
};
use crate::repr::symbols::{Symbol, SymbolValue, SymbolVisibility};
use plinky_diagnostics::widgets::{HexDump, Table, Text, Widget, WidgetGroup};
use plinky_diagnostics::{Diagnostic, DiagnosticKind};
use plinky_elf::ids::serial::{SectionId, SerialIds, SymbolId};
use plinky_elf::{ElfDeduplication, ElfObject, ElfPermissions};
use std::collections::BTreeSet;

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
            table.add_row(["Section", "Source object", "Memory address"]);

            for section in object.sections.values() {
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
                            table.add_row([section_name(object, *id), source.to_string(), address]);
                        }
                    }
                    SectionContent::Uninitialized(parts) => {
                        for (id, part) in parts {
                            table.add_row([
                                section_name(object, *id),
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

fn render_object<T: RenderLayout>(message: &str, object: &Object<T>) {
    let diagnostic = Diagnostic::new(DiagnosticKind::DebugPrint, message)
        .add(render_env(object))
        .add_iter(
            object.sections.values().flat_map(|section| render_section_group(object, section)),
        )
        .add(render_symbols(object, object.symbols.iter()));
    eprintln!("{diagnostic}\n");
}

fn render_env<T>(object: &Object<T>) -> Text {
    Text::new(format!(
        "class: {:?}, endian: {:?}, abi: {:?}, machine: {:?}",
        object.env.class, object.env.endian, object.env.abi, object.env.machine
    ))
}

fn render_section_group<T: RenderLayout>(
    object: &Object<T>,
    section: &Section<T>,
) -> Vec<Box<dyn Widget>> {
    match &section.content {
        SectionContent::Data(data) => data
            .parts
            .iter()
            .map(|(&id, part)| match part {
                DataSectionPart::Real(real) => {
                    render_data_section(object, id, &section.perms, data.deduplication, real)
                }
                DataSectionPart::DeduplicationFacade(facade) => {
                    render_deduplication_facade(object, id, facade)
                }
            })
            .collect(),
        SectionContent::Uninitialized(uninitialized) => uninitialized
            .iter()
            .map(|(&id, part)| render_uninitialized_section(object, id, &section.perms, part))
            .collect(),
    }
}

fn render_data_section<T: RenderLayout>(
    object: &Object<T>,
    id: SectionId,
    perms: &ElfPermissions,
    deduplication: ElfDeduplication,
    part: &DataSectionPartReal<T>,
) -> Box<dyn Widget> {
    let deduplication = match deduplication {
        ElfDeduplication::Disabled => None,
        ElfDeduplication::ZeroTerminatedStrings => {
            Some(Text::new("zero-terminated strings should be deduplicated"))
        }
        ElfDeduplication::FixedSizeChunks { size } => {
            Some(Text::new(format!("fixed chunks of size {size:#x} should be deduplicated")))
        }
    };

    let relocations = if part.relocations.is_empty() {
        None
    } else {
        let mut table = Table::new();
        table.set_title("Relocations:");
        table.add_row(["Type", "Symbol", "Offset", "Addend"]);
        for relocation in &part.relocations {
            table.add_row([
                format!("{:?}", relocation.relocation_type),
                symbol_name(object, relocation.symbol),
                format!("{:#x}", relocation.offset),
                relocation.addend.map(|a| format!("{a:#x}")).unwrap_or_else(String::new),
            ])
        }
        Some(table)
    };

    Box::new(
        WidgetGroup::new()
            .name(format!(
                "section {} ({}) in {}",
                section_name(object, id),
                permissions(perms),
                part.source
            ))
            .add_iter(deduplication)
            .add_iter(part.layout.render_layoyt())
            .add(HexDump::new(part.bytes.0.clone()))
            .add_iter(relocations),
    )
}

fn render_deduplication_facade<T>(
    object: &Object<T>,
    id: SectionId,
    facade: &DeduplicationFacade,
) -> Box<dyn Widget> {
    let target = section_name(object, facade.section_id);

    let mut table = Table::new();
    table.set_title(format!(
        "deduplication facade {} in {}",
        section_name(object, id),
        facade.source
    ));
    table.add_row(["From", "To"]);
    for (from, to) in &facade.offset_map {
        table.add_row([format!("{from:#x}"), format!("{target} + {to:#x}")]);
    }

    Box::new(table)
}

fn render_uninitialized_section<T: RenderLayout>(
    object: &Object<T>,
    id: SectionId,
    perms: &ElfPermissions,
    section: &UninitializedSectionPart<T>,
) -> Box<dyn Widget> {
    Box::new(
        WidgetGroup::new()
            .name(format!(
                "uninitialized section {} ({}) in {}",
                section_name(object, id),
                permissions(perms),
                section.source
            ))
            .add(Text::new(format!("length: {:#x}", section.len)))
            .add_iter(section.layout.render_layoyt()),
    )
}

trait RenderLayout
where
    Self: Sized,
{
    fn render_layoyt(&self) -> Option<Text>;
}

impl RenderLayout for () {
    fn render_layoyt(&self) -> Option<Text> {
        None
    }
}

impl RenderLayout for SectionLayout {
    fn render_layoyt(&self) -> Option<Text> {
        Some(Text::new(format!("address: {:#x}", self.address)))
    }
}

fn render_symbols<'a, T>(
    object: &Object<T>,
    symbols: impl Iterator<Item = (SymbolId, &'a Symbol)>,
) -> Table {
    let mut symbols = symbols.collect::<Vec<_>>();
    symbols.sort_by_key(|(_, symbol)| symbol.name);

    let mut table = Table::new();
    table.set_title("Symbols:");
    table.add_row(["Name", "Source", "Visibility", "Value"]);
    for (id, symbol) in symbols {
        let visibility = match symbol.visibility {
            SymbolVisibility::Local => "local",
            SymbolVisibility::Global { weak: true } => "global (weak)",
            SymbolVisibility::Global { weak: false } => "global",
        };
        let value = match symbol.value {
            SymbolValue::Absolute { value } => format!("{value:#x}"),
            SymbolValue::SectionRelative { section, offset } => {
                format!("{} + {offset:#x}", section_name(object, section))
            }
            SymbolValue::Undefined => "<undefined>".into(),
        };
        table.add_row([&symbol_name(object, id), &symbol.span.to_string(), visibility, &value]);
    }
    table
}

fn render<T: Widget + 'static>(message: &str, widget: T) {
    let diagnostic = Diagnostic::new(DiagnosticKind::DebugPrint, message).add(widget);
    eprintln!("{diagnostic}\n");
}

fn permissions(perms: &ElfPermissions) -> String {
    let mut output = String::new();
    if perms.read {
        output.push('r');
    }
    if perms.write {
        output.push('w');
    }
    if perms.execute {
        output.push('x');
    }
    if output.is_empty() {
        "no perms".into()
    } else {
        format!("perms: {output}")
    }
}

fn section_name<T>(object: &Object<T>, id: SectionId) -> String {
    object
        .section_ids_to_names
        .get(&id)
        .map(|name| format!("{}#{}", name.resolve(), id.idx()))
        .unwrap_or_else(|| "<unknown section>".into())
}

fn symbol_name<T>(object: &Object<T>, id: SymbolId) -> String {
    object
        .symbols
        .get(id)
        .map(|symbol| {
            let name = symbol.name.resolve();
            match (name.as_str(), &symbol.value) {
                ("", SymbolValue::SectionRelative { section, offset: 0 }) => {
                    format!("<section {}>", section_name(object, *section))
                }
                ("", _) => format!("<symbol#{}>", symbol.id.idx()),
                (name, _) => format!("{}#{}", name, symbol.id.idx()),
            }
        })
        .unwrap_or_else(|_| "<unknown symbol>".into())
}
