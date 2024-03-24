use crate::cli::DebugPrint;
use crate::linker::LinkerCallbacks;
use crate::passes::deduplicate::Deduplication;
use crate::passes::layout::{Layout, SectionLayout, SegmentType};
use crate::repr::object::{
    DataSectionPart, Object, Section, SectionContent, UninitializedSectionPart,
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
    fn on_inputs_loaded(&self, object: &Object) {
        if self.print.contains(&DebugPrint::LoadedObject) {
            render_object("loaded object", object, None);
        }
    }

    fn on_layout_calculated(&self, object: &Object, layout: &Layout) {
        if self.print.contains(&DebugPrint::Layout) {
            let mut sections = Table::new();
            sections.set_title("Sections:");
            sections.add_row(["Section", "Source object", "Memory address"]);

            let render_layout = |id| match layout.of_section(id) {
                SectionLayout::Allocated { address } => format!("{address:#x}"),
                SectionLayout::NotAllocated => "not allocated".to_string(),
            };

            for section in object.sections.values() {
                match &section.content {
                    SectionContent::Data(data) => {
                        for (&id, part) in &data.parts {
                            sections.add_row([
                                section_name(object, id),
                                part.source.to_string(),
                                render_layout(id),
                            ]);
                        }
                    }
                    SectionContent::Uninitialized(parts) => {
                        for (&id, part) in parts {
                            sections.add_row([
                                section_name(object, id),
                                part.source.to_string(),
                                render_layout(id),
                            ]);
                        }
                    }
                }
            }

            let mut segments = Table::new();
            segments.set_title("Segments:");
            segments.add_row(["Start", "Size", "Align", "Type", "Permissions", "Sections"]);
            for segment in layout.iter_segments() {
                segments.add_row([
                    format!("{:#x}", segment.start),
                    format!("{:#x}", segment.len),
                    format!("{:#x}", segment.align),
                    match segment.type_ {
                        SegmentType::Program => "program".into(),
                        SegmentType::Uninitialized => "uninit".into(),
                    },
                    format!("{:?}", segment.perms),
                    segment
                        .sections
                        .iter()
                        .map(|id| section_name(object, *id))
                        .collect::<Vec<_>>()
                        .join("\n"),
                ]);
            }

            render(
                Diagnostic::new(DiagnosticKind::DebugPrint, "calculated layout")
                    .add(sections)
                    .add(segments)
                    .add_iter(layout.iter_deduplications().map(|(id, deduplication)| {
                        render_deduplication(object, id, deduplication)
                    })),
            );
        }
    }

    fn on_relocations_applied(&self, object: &Object, layout: &Layout) {
        if self.print.contains(&DebugPrint::RelocatedObject) {
            render_object("object after relocations are applied", object, Some(layout));
        }
    }

    fn on_elf_built(&self, elf: &ElfObject<SerialIds>) {
        if self.print.contains(&DebugPrint::FinalElf) {
            render(
                Diagnostic::new(DiagnosticKind::DebugPrint, "built elf")
                    .add(Text::new(format!("{elf:#x?}"))),
            );
        }
    }
}

fn render_object(message: &str, object: &Object, layout: Option<&Layout>) {
    render(
        Diagnostic::new(DiagnosticKind::DebugPrint, message)
            .add(render_env(object))
            .add_iter(
                object
                    .sections
                    .values()
                    .flat_map(|section| render_section_group(object, layout, section)),
            )
            .add(render_symbols(object, object.symbols.iter())),
    );
}

fn render_env(object: &Object) -> Text {
    Text::new(format!(
        "class: {:?}, endian: {:?}, abi: {:?}, machine: {:?}",
        object.env.class, object.env.endian, object.env.abi, object.env.machine
    ))
}

fn render_section_group(
    object: &Object,
    layout: Option<&Layout>,
    section: &Section,
) -> Vec<Box<dyn Widget>> {
    match &section.content {
        SectionContent::Data(data) => data
            .parts
            .iter()
            .map(|(&id, part)| {
                render_data_section(object, layout, id, &section.perms, data.deduplication, part)
            })
            .collect(),
        SectionContent::Uninitialized(uninitialized) => uninitialized
            .iter()
            .map(|(&id, part)| {
                render_uninitialized_section(object, layout, id, &section.perms, part)
            })
            .collect(),
    }
}

fn render_data_section(
    object: &Object,
    layout: Option<&Layout>,
    id: SectionId,
    perms: &ElfPermissions,
    deduplication: ElfDeduplication,
    part: &DataSectionPart,
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
            .add_iter(render_layout(layout, id))
            .add(HexDump::new(part.bytes.0.clone()))
            .add_iter(relocations),
    )
}

fn render_uninitialized_section(
    object: &Object,
    layout: Option<&Layout>,
    id: SectionId,
    perms: &ElfPermissions,
    section: &UninitializedSectionPart,
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
            .add_iter(render_layout(layout, id)),
    )
}

fn render_symbols<'a>(
    object: &Object,
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

fn render_deduplication(
    object: &Object,
    id: SectionId,
    deduplication: &Deduplication,
) -> Box<dyn Widget> {
    let target = section_name(object, deduplication.target);

    let mut table = Table::new();
    table.set_title(format!(
        "deduplication facade {} in {}",
        section_name(object, id),
        deduplication.source
    ));
    table.add_row(["From", "To"]);
    for (from, to) in &deduplication.map {
        table.add_row([format!("{from:#x}"), format!("{target} + {to:#x}")]);
    }

    Box::new(table)
}

fn render_layout(layout: Option<&Layout>, id: SectionId) -> Option<Text> {
    layout.map(|layout| match layout.of_section(id) {
        SectionLayout::Allocated { address } => Text::new(format!("address: {address:#x}")),
        SectionLayout::NotAllocated => Text::new("not allocated in the resulting memory"),
    })
}

fn render(diagnostic: Diagnostic) {
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

fn section_name(object: &Object, id: SectionId) -> String {
    object
        .section_ids_to_names
        .get(&id)
        .map(|name| format!("{}#{}", name.resolve(), id.idx()))
        .unwrap_or_else(|| "<unknown section>".into())
}

fn symbol_name(object: &Object, id: SymbolId) -> String {
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
