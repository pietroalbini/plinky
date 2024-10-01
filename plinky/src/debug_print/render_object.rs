use crate::debug_print::filters::ObjectsFilter;
use crate::debug_print::utils::{permissions, section_name, symbol_name};
use crate::repr::object::Object;
use crate::repr::relocations::Relocation;
use crate::repr::sections::{
    DataSection, DynamicSection, RelocationsSection, Section, SectionContent, StringsSection,
    SymbolsSection, SysvHashSection, UninitializedSection,
};
use crate::repr::symbols::views::{AllSymbols, DynamicSymbolTable, SymbolsView};
use crate::repr::symbols::{SymbolType, SymbolValue, SymbolVisibility};
use plinky_diagnostics::widgets::{HexDump, Table, Text, Widget, WidgetGroup};
use plinky_diagnostics::{Diagnostic, DiagnosticKind};
use plinky_elf::ids::serial::{SectionId, SerialIds};
use plinky_elf::writer::layout::Layout;
use plinky_elf::ElfDeduplication;

pub(super) fn render_object(
    message: &str,
    filter: &ObjectsFilter,
    object: &Object,
    layout: Option<&Layout<SerialIds>>,
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
        .add_iter(filter.symbols.then(|| render_symbols(object, "Symbols:", &AllSymbols)).flatten())
        .add_iter(
            filter
                .dynamic
                .then(|| render_symbols(object, "Dynamic symbols:", &DynamicSymbolTable))
                .flatten(),
        )
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

fn render_section(
    object: &Object,
    layout: Option<&Layout<SerialIds>>,
    section: &Section,
) -> Box<dyn Widget> {
    match &section.content {
        SectionContent::Data(data) => render_data_section(object, layout, section, data),
        SectionContent::Uninitialized(uninit) => {
            render_uninitialized_section(object, layout, section, uninit)
        }
        SectionContent::Strings(strings) => render_strings_section(object, section, strings),
        SectionContent::Symbols(symbols) => render_symbols_section(object, section, symbols),
        SectionContent::SysvHash(sysv) => render_sysv_hash_section(object, section, sysv),
        SectionContent::Relocations(relocations) => {
            render_relocations_section(object, section, relocations)
        }
        SectionContent::Dynamic(dynamic) => render_dynamic_section(object, section, dynamic),
        SectionContent::SectionNames => render_section_names_section(object, section),
    }
}

fn render_data_section(
    object: &Object,
    layout: Option<&Layout<SerialIds>>,
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
                permissions(&data.perms),
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
    layout: Option<&Layout<SerialIds>>,
    section: &Section,
    uninit: &UninitializedSection,
) -> Box<dyn Widget> {
    Box::new(
        WidgetGroup::new()
            .name(format!(
                "uninitialized section {} ({}) in {}",
                section_name(object, section.id),
                permissions(&uninit.perms),
                section.source
            ))
            .add(Text::new(format!("length: {}", uninit.len)))
            .add_iter(render_layout(layout, section.id)),
    )
}

fn render_symbols<'a>(object: &Object, title: &str, view: &dyn SymbolsView) -> Option<Table> {
    let mut symbols = object.symbols.iter(view).collect::<Vec<_>>();
    if symbols.len() <= 1 {
        return None;
    }
    symbols.sort_by_key(|(_, symbol)| symbol.name());

    let mut table = Table::new();
    table.set_title(title);
    table.add_row(["Name", "Type", "Source", "Visibility", "Value"]);
    for (id, symbol) in symbols {
        let type_ = match symbol.type_() {
            SymbolType::NoType => "none",
            SymbolType::Function => "function",
            SymbolType::Object => "object",
            SymbolType::Section => "section",
        };
        let visibility = match symbol.visibility() {
            SymbolVisibility::Local => "local",
            SymbolVisibility::Global { weak: true, hidden: true } => "global (weak, hidden)",
            SymbolVisibility::Global { weak: true, hidden: false } => "global (weak)",
            SymbolVisibility::Global { weak: false, hidden: true } => "global (hidden)",
            SymbolVisibility::Global { weak: false, hidden: false } => "global",
        };
        let value = match symbol.value() {
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
            &symbol.span().to_string(),
            visibility,
            &value,
        ]);
    }
    Some(table)
}

fn render_layout(layout: Option<&Layout<SerialIds>>, id: SectionId) -> Option<Text> {
    layout.map(|layout| match &layout.metadata_of_section(&id).memory {
        Some(mem) => Text::new(format!("address: {}", mem.address)),
        None => Text::new("not allocated in the resulting memory"),
    })
}

fn render_strings_section(
    object: &Object,
    section: &Section,
    strings: &StringsSection,
) -> Box<dyn Widget> {
    Box::new(
        section_widget(object, section, "string table")
            .add(Text::new(format!("symbol names for: {}", strings.symbol_names_view()))),
    )
}

fn render_symbols_section(
    object: &Object,
    section: &Section,
    symbols: &SymbolsSection,
) -> Box<dyn Widget> {
    Box::new(section_widget(object, section, "symbols table").add(Text::new(format!(
        "view: {}\nstrings: {}",
        symbols.view,
        section_name(object, symbols.strings)
    ))))
}

fn render_sysv_hash_section(
    object: &Object,
    section: &Section,
    sysv: &SysvHashSection,
) -> Box<dyn Widget> {
    Box::new(section_widget(object, section, "SysV hash").add(Text::new(format!(
        "view: {}\nsymbols: {}",
        sysv.view,
        section_name(object, sysv.symbols)
    ))))
}

fn render_relocations_section(
    object: &Object,
    section: &Section,
    relocations: &RelocationsSection,
) -> Box<dyn Widget> {
    Box::new(
        section_widget(object, section, "relocations")
            .add(Text::new(format!(
                "applies to section: {}\nsymbol table: {}",
                section_name(object, relocations.section()),
                section_name(object, relocations.symbols_table())
            )))
            .add(render_relocations(object, "Relocations:", relocations.relocations())),
    )
}

fn render_dynamic_section(
    object: &Object,
    section: &Section,
    dynamic: &DynamicSection,
) -> Box<dyn Widget> {
    Box::new(
        section_widget(object, section, "dynamic")
            .add(Text::new(format!("strings table: {}", section_name(object, dynamic.strings())))),
    )
}

fn render_section_names_section(object: &Object, section: &Section) -> Box<dyn Widget> {
    Box::new(section_widget(object, section, "section names").add(Text::new("section names")))
}

fn section_widget(object: &Object, section: &Section, kind: &str) -> WidgetGroup {
    WidgetGroup::new().name(format!(
        "{kind} section {} in {}",
        section_name(object, section.id),
        section.source
    ))
}
