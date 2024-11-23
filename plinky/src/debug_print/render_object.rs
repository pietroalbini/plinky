use crate::debug_print::filters::ObjectsFilter;
use crate::debug_print::names::Names;
use crate::debug_print::utils::permissions;
use crate::repr::object::Object;
use crate::repr::relocations::{Relocation, RelocationAddend};
use crate::repr::sections::{
    DataSection, DynamicSection, NotesSection, RelocationsSection, Section, SectionContent,
    SectionId, StringsSection, SymbolsSection, SysvHashSection, UninitializedSection,
};
use crate::repr::symbols::views::{AllSymbols, DynamicSymbolTable, SymbolsView};
use crate::repr::symbols::{SymbolType, SymbolValue, SymbolVisibility};
use plinky_diagnostics::widgets::{HexDump, Table, Text, Widget, WidgetGroup};
use plinky_diagnostics::{Diagnostic, DiagnosticKind};
use plinky_elf::render_elf::render_note;
use plinky_elf::writer::layout::Layout;
use plinky_elf::ElfDeduplication;

pub(super) fn render_object(
    message: &str,
    filter: &ObjectsFilter,
    object: &Object,
    layout: Option<&Layout<SectionId>>,
) -> Diagnostic {
    let names = Names::new(object);

    let mut sorted_sections = object.sections.iter().collect::<Vec<_>>();
    sorted_sections.sort_by_key(|section| (section.name, section.id));

    Diagnostic::new(DiagnosticKind::DebugPrint, message)
        .add_iter(filter.env.then(|| render_env(object)))
        .add_iter(
            sorted_sections
                .iter()
                .filter(|section| filter.section(&section.name.resolve()))
                .map(|section| render_section(&names, layout, section)),
        )
        .add_iter(
            filter
                .symbols
                .then(|| render_symbols(object, &names, "Symbols:", &AllSymbols))
                .flatten(),
        )
        .add_iter(
            filter
                .dynamic
                .then(|| render_symbols(object, &names, "Dynamic symbols:", &DynamicSymbolTable))
                .flatten(),
        )
        .add_iter(filter.inputs.then(|| render_inputs(object)))
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
    names: &Names,
    layout: Option<&Layout<SectionId>>,
    section: &Section,
) -> Box<dyn Widget> {
    match &section.content {
        SectionContent::Data(data) => render_data_section(names, layout, section, data),
        SectionContent::Uninitialized(uninit) => {
            render_uninitialized_section(names, layout, section, uninit)
        }
        SectionContent::Strings(strings) => render_strings_section(names, section, strings),
        SectionContent::Symbols(symbols) => render_symbols_section(names, section, symbols),
        SectionContent::SysvHash(sysv) => render_sysv_hash_section(names, section, sysv),
        SectionContent::Relocations(relocations) => {
            render_relocations_section(names, section, relocations)
        }
        SectionContent::Dynamic(dynamic) => render_dynamic_section(names, section, dynamic),
        SectionContent::Notes(notes) => render_notes_section(names, section, notes),
        SectionContent::SectionNames => render_section_names_section(names, section),
    }
}

fn render_data_section(
    names: &Names,
    layout: Option<&Layout<SectionId>>,
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
        Some(render_relocations(names, "Relocations:", &data.relocations))
    };

    Box::new(
        WidgetGroup::new()
            .name(format!(
                "section {} ({}) in {}",
                names.section(section.id),
                permissions(&data.perms),
                section.source
            ))
            .add_iter(deduplication)
            .add_iter(render_layout(layout, section.id))
            .add(HexDump::new(data.bytes.clone()))
            .add_iter(relocations),
    )
}

fn render_relocations(names: &Names, title: &str, relocations: &[Relocation]) -> Box<dyn Widget> {
    let mut table = Table::new();
    table.set_title(title);
    table.add_row(["Type", "Symbol", "Offset", "Addend"]);
    for relocation in relocations {
        table.add_row([
            format!("{:?}", relocation.type_),
            names.symbol(relocation.symbol).into(),
            format!("{}", relocation.offset),
            match &relocation.addend {
                RelocationAddend::Inline => "<inline>".into(),
                RelocationAddend::Explicit(offset) => offset.to_string(),
            },
        ])
    }
    Box::new(table)
}

fn render_uninitialized_section(
    names: &Names,
    layout: Option<&Layout<SectionId>>,
    section: &Section,
    uninit: &UninitializedSection,
) -> Box<dyn Widget> {
    Box::new(
        WidgetGroup::new()
            .name(format!(
                "uninitialized section {} ({}) in {}",
                names.section(section.id),
                permissions(&uninit.perms),
                section.source
            ))
            .add(Text::new(format!("length: {}", uninit.len)))
            .add_iter(render_layout(layout, section.id)),
    )
}

fn render_symbols<'a>(
    object: &Object,
    names: &Names,
    title: &str,
    view: &dyn SymbolsView,
) -> Option<Table> {
    let mut symbols = object.symbols.iter(view).collect::<Vec<_>>();
    if symbols.len() <= 1 {
        return None;
    }
    symbols.sort_by_key(|symbol| symbol.name());

    let mut table = Table::new();
    table.set_title(title);
    table.add_row(["Name", "Type", "Source", "Visibility", "Value"]);
    for symbol in symbols {
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
                format!("{} + {offset}", names.section(section))
            }
            SymbolValue::SectionVirtualAddress { section, memory_address } => {
                format!("{memory_address} (in {})", names.section(section))
            }
            SymbolValue::ExternallyDefined => "<externally defined>".into(),
            SymbolValue::SectionNotLoaded => "<section not loaded>".into(),
            SymbolValue::Undefined => "<undefined>".into(),
            SymbolValue::Null => "<null>".into(),
        };
        table.add_row([
            names.symbol(symbol.id()),
            type_,
            &symbol.span().to_string(),
            visibility,
            &value,
        ]);
    }
    Some(table)
}

fn render_layout(layout: Option<&Layout<SectionId>>, id: SectionId) -> Option<Text> {
    layout.map(|layout| match &layout.metadata_of_section(&id).memory {
        Some(mem) => Text::new(format!("address: {}", mem.address)),
        None => Text::new("not allocated in the resulting memory"),
    })
}

fn render_strings_section(
    names: &Names,
    section: &Section,
    strings: &StringsSection,
) -> Box<dyn Widget> {
    let mut custom_count = 0;
    let mut custom = Table::new();
    custom.set_title("Additional strings:");
    for (_, string) in strings.iter_custom_strings() {
        custom_count += 1;
        custom.add_row([string]);
    }

    Box::new(
        section_widget(names, section, "string table")
            .add(Text::new(format!("symbol names for: {}", strings.symbol_names_view())))
            .add_iter(Some(custom).filter(|_| custom_count > 0)),
    )
}

fn render_symbols_section(
    names: &Names,
    section: &Section,
    symbols: &SymbolsSection,
) -> Box<dyn Widget> {
    Box::new(section_widget(names, section, "symbols table").add(Text::new(format!(
        "view: {}\nstrings: {}",
        symbols.view,
        names.section(symbols.strings)
    ))))
}

fn render_sysv_hash_section(
    names: &Names,
    section: &Section,
    sysv: &SysvHashSection,
) -> Box<dyn Widget> {
    Box::new(section_widget(names, section, "SysV hash").add(Text::new(format!(
        "view: {}\nsymbols: {}",
        sysv.view,
        names.section(sysv.symbols)
    ))))
}

fn render_relocations_section(
    names: &Names,
    section: &Section,
    relocations: &RelocationsSection,
) -> Box<dyn Widget> {
    Box::new(
        section_widget(names, section, "relocations")
            .add(Text::new(format!(
                "applies to section: {}\nsymbol table: {}",
                names.section(relocations.section()),
                names.section(relocations.symbols_table())
            )))
            .add(render_relocations(names, "Relocations:", relocations.relocations())),
    )
}

fn render_dynamic_section(
    names: &Names,
    section: &Section,
    dynamic: &DynamicSection,
) -> Box<dyn Widget> {
    Box::new(
        section_widget(names, section, "dynamic")
            .add(Text::new(format!("strings table: {}", names.section(dynamic.strings())))),
    )
}

fn render_notes_section(names: &Names, section: &Section, notes: &NotesSection) -> Box<dyn Widget> {
    Box::new(section_widget(names, section, "notes").add_iter(notes.notes.iter().map(render_note)))
}

fn render_section_names_section(names: &Names, section: &Section) -> Box<dyn Widget> {
    Box::new(section_widget(names, section, "section names").add(Text::new("section names")))
}

fn render_inputs(object: &Object) -> Box<dyn Widget> {
    let mut result: Vec<Box<dyn Widget>> = Vec::new();

    for input in &object.inputs {
        let mut title = input.span.to_string();
        if input.shared_object {
            title.push_str(" (shared object)");
        }

        let mut table = Table::new();
        table.set_title(title.clone());

        if let Some(isa) = &input.gnu_properties.x86_isa_used {
            table.add_row(["X86 ISA used".to_string(), isa.to_string()]);
        }
        if let Some(features2) = &input.gnu_properties.x86_features_2_used {
            table.add_row(["x86 features 2 used".to_string(), features2.to_string()]);
        }

        if table.is_empty() {
            result.push(Box::new(Text::new(title)));
        } else {
            result.push(Box::new(table));
        }
    }

    Box::new(WidgetGroup::new().name("inputs").add_iter(result.into_iter()))
}

fn section_widget(names: &Names, section: &Section, kind: &str) -> WidgetGroup {
    WidgetGroup::new().name(format!(
        "{kind} section {} in {}",
        names.section(section.id),
        section.source
    ))
}
