use crate::debug_print::filters::ObjectsFilter;
use crate::debug_print::utils::{permissions, section_name, symbol_name};
use crate::passes::layout::{Layout, SectionLayout};
use crate::repr::object::Object;
use crate::repr::relocations::Relocation;
use crate::repr::sections::{DataSection, Section, SectionContent, UninitializedSection};
use crate::repr::symbols::{Symbol, SymbolType, SymbolValue, SymbolVisibility};
use plinky_diagnostics::widgets::{HexDump, Table, Text, Widget, WidgetGroup};
use plinky_diagnostics::{Diagnostic, DiagnosticKind};
use plinky_elf::ids::serial::{SectionId, SymbolId};
use plinky_elf::ElfDeduplication;

pub(super) fn render_object(
    message: &str,
    filter: &ObjectsFilter,
    object: &Object,
    layout: Option<&Layout>,
) -> Diagnostic {
    let mut sorted_sections = object.sections.iter().collect::<Vec<_>>();
    sorted_sections.sort_by_key(|section| (section.name, section.id));

    Diagnostic::new(DiagnosticKind::DebugPrint, message)
        .add_iter(filter.env.then(|| render_env(object)))
        .add_iter(
            sorted_sections
                .iter()
                .filter(|section| filter.section(&section.name.resolve()))
                .map(|section| render_section(object, layout, section)),
        )
        .add_iter(filter.symbols.then(|| render_symbols(object, "Symbols:", object.symbols.iter())))
        .add_iter((filter.dynamic && object.symbols.has_dynamic_symbols()).then(|| {
            render_symbols(object, "Dynamic symbols:", object.symbols.iter_dynamic_symbols())
        }))
        .add_iter((filter.dynamic && !object.dynamic_relocations.is_empty()).then(|| {
            render_relocations(object, "Dynamic relocations:", &object.dynamic_relocations)
        }))
}

fn render_env(object: &Object) -> Text {
    let mut content = format!(
        "class: {:?}, endian: {:?}, abi: {:?}, machine: {:?}",
        object.env.class, object.env.endian, object.env.abi, object.env.machine
    );
    if object.gnu_stack_section_ignored {
        content.push_str(", .note.GNU-stack sections ignored");
    }
    Text::new(content)
}

fn render_section(object: &Object, layout: Option<&Layout>, section: &Section) -> Box<dyn Widget> {
    match &section.content {
        SectionContent::Data(data) => render_data_section(object, layout, section, data),
        SectionContent::Uninitialized(uninit) => {
            render_uninitialized_section(object, layout, section, uninit)
        }
    }
}

fn render_data_section(
    object: &Object,
    layout: Option<&Layout>,
    section: &Section,
    data: &DataSection,
) -> Box<dyn Widget> {
    let deduplication = match data.deduplication {
        ElfDeduplication::Disabled => None,
        ElfDeduplication::ZeroTerminatedStrings => {
            Some(Text::new("zero-terminated strings should be deduplicated"))
        }
        ElfDeduplication::FixedSizeChunks { size } => {
            Some(Text::new(format!("fixed chunks of size {size:#x} should be deduplicated")))
        }
    };

    let relocations = if data.relocations.is_empty() {
        None
    } else {
        Some(render_relocations(object, "Relocations:", &data.relocations))
    };

    Box::new(
        WidgetGroup::new()
            .name(format!(
                "section {} ({}) in {}",
                section_name(object, section.id),
                permissions(&section.perms),
                section.source
            ))
            .add_iter(deduplication)
            .add_iter(render_layout(layout, section.id))
            .add(HexDump::new(data.bytes.clone()))
            .add_iter(relocations),
    )
}

fn render_relocations(object: &Object, title: &str, relocations: &[Relocation]) -> Box<dyn Widget> {
    let mut table = Table::new();
    table.set_title(title);
    table.add_row(["Type", "Symbol", "Offset", "Addend"]);
    for relocation in relocations {
        table.add_row([
            format!("{:?}", relocation.type_),
            symbol_name(object, relocation.symbol),
            format!("{}", relocation.offset),
            relocation.addend.map(|a| format!("{a}")).unwrap_or_else(String::new),
        ])
    }
    Box::new(table)
}

fn render_uninitialized_section(
    object: &Object,
    layout: Option<&Layout>,
    section: &Section,
    uninit: &UninitializedSection,
) -> Box<dyn Widget> {
    Box::new(
        WidgetGroup::new()
            .name(format!(
                "uninitialized section {} ({}) in {}",
                section_name(object, section.id),
                permissions(&section.perms),
                section.source
            ))
            .add(Text::new(format!("length: {:#x}", uninit.len)))
            .add_iter(render_layout(layout, section.id)),
    )
}

fn render_symbols<'a>(
    object: &Object,
    title: &str,
    symbols: impl Iterator<Item = (SymbolId, &'a Symbol)>,
) -> Table {
    let mut symbols = symbols.collect::<Vec<_>>();
    symbols.sort_by_key(|(_, symbol)| symbol.name);

    let mut table = Table::new();
    table.set_title(title);
    table.add_row(["Name", "Type", "Source", "Visibility", "Value"]);
    for (id, symbol) in symbols {
        let type_ = match symbol.type_ {
            SymbolType::NoType => "none",
            SymbolType::Function => "function",
            SymbolType::Object => "object",
            SymbolType::Section => "section",
        };
        let visibility = match symbol.visibility {
            SymbolVisibility::Local => "local",
            SymbolVisibility::Global { weak: true, hidden: true } => "global (weak, hidden)",
            SymbolVisibility::Global { weak: true, hidden: false } => "global (weak)",
            SymbolVisibility::Global { weak: false, hidden: true } => "global (hidden)",
            SymbolVisibility::Global { weak: false, hidden: false } => "global",
        };
        let value = match symbol.value {
            SymbolValue::Absolute { value } => format!("{value}"),
            SymbolValue::SectionRelative { section, offset } => {
                format!("{} + {offset}", section_name(object, section))
            }
            SymbolValue::SectionVirtualAddress { section, memory_address } => {
                format!("{memory_address} (in {})", section_name(object, section))
            }
            SymbolValue::Undefined => "<undefined>".into(),
            SymbolValue::Null => "<null>".into(),
        };
        table.add_row([
            symbol_name(object, id).as_str(),
            type_,
            &symbol.span.to_string(),
            visibility,
            &value,
        ]);
    }
    table
}

fn render_layout(layout: Option<&Layout>, id: SectionId) -> Option<Text> {
    layout.map(|layout| match layout.of_section(id) {
        SectionLayout::Allocated { address } => Text::new(format!("address: {address}")),
        SectionLayout::NotAllocated => Text::new("not allocated in the resulting memory"),
    })
}
