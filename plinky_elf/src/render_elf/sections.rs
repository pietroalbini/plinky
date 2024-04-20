use crate::ids::serial::{SectionId, SerialIds};
use crate::render_elf::utils::{render_perms, section_name, symbol_name};
use crate::{
    ElfDeduplication, ElfNote, ElfNotesTable, ElfObject, ElfProgramSection, ElfRelocationsTable,
    ElfSection, ElfSectionContent, ElfStringTable, ElfSymbolBinding, ElfSymbolDefinition,
    ElfSymbolTable, ElfSymbolType, ElfUninitializedSection, ElfUnknownSection,
};
use plinky_diagnostics::widgets::{HexDump, Table, Text, Widget, WidgetGroup};

pub(super) fn render_section(
    object: &ElfObject<SerialIds>,
    id: SectionId,
    section: &ElfSection<SerialIds>,
) -> impl Widget {
    let content: Vec<Box<dyn Widget>> = match &section.content {
        ElfSectionContent::Null => vec![Box::new(Text::new("empty section"))],
        ElfSectionContent::Program(program) => render_section_program(program),
        ElfSectionContent::Uninitialized(uninit) => render_section_uninit(uninit),
        ElfSectionContent::SymbolTable(symbols) => render_section_symbols(object, id, symbols),
        ElfSectionContent::StringTable(strings) => render_section_strings(strings),
        ElfSectionContent::RelocationsTable(relocs) => render_section_relocs(object, relocs),
        ElfSectionContent::Note(notes) => render_section_notes(notes),
        ElfSectionContent::Unknown(unknown) => render_section_unknown(unknown),
    };

    WidgetGroup::new()
        .name(format!(
            "section {} (address: {:#x})",
            section_name(object, id),
            section.memory_address
        ))
        .add_iter(content)
}

fn render_section_program(program: &ElfProgramSection) -> Vec<Box<dyn Widget>> {
    let mut intro = format!("program data | permissions: {}", render_perms(&program.perms));

    match program.deduplication {
        ElfDeduplication::Disabled => {}
        ElfDeduplication::ZeroTerminatedStrings => {
            intro.push_str(" | deduplicating zero-terminated strings");
        }
        ElfDeduplication::FixedSizeChunks { size } => {
            intro.push_str(&format!(" | deduplicating chunks of size {size:#x}"));
        }
    }

    vec![Box::new(Text::new(intro.trim())), Box::new(HexDump::new(program.raw.0.as_slice()))]
}

fn render_section_uninit(uninit: &ElfUninitializedSection) -> Vec<Box<dyn Widget>> {
    vec![Box::new(Text::new(format!(
        "uninitialized | len: {:#x} | permissions: {}",
        uninit.len,
        render_perms(&uninit.perms)
    )))]
}

fn render_section_symbols(
    object: &ElfObject<SerialIds>,
    section_id: SectionId,
    symbols: &ElfSymbolTable<SerialIds>,
) -> Vec<Box<dyn Widget>> {
    let mut table = Table::new();
    table.set_title("Symbol table:");
    table.add_row(["Name", "Binding", "Type", "Definition", "Value", "Size"]);
    for (id, symbol) in &symbols.symbols {
        table.add_row([
            symbol_name(object, section_id, *id),
            match symbol.binding {
                ElfSymbolBinding::Local => "Local".into(),
                ElfSymbolBinding::Global => "Global".into(),
                ElfSymbolBinding::Weak => "Weak".into(),
                ElfSymbolBinding::Unknown(unknown) => format!("<unknown: {unknown:#x}>"),
            },
            match symbol.type_ {
                ElfSymbolType::NoType => "-".into(),
                ElfSymbolType::Object => "Object".into(),
                ElfSymbolType::Function => "Function".into(),
                ElfSymbolType::Section => "Section".into(),
                ElfSymbolType::File => "File".into(),
                ElfSymbolType::Unknown(unknown) => format!("<unknown: {unknown:#x}>"),
            },
            match symbol.definition {
                ElfSymbolDefinition::Undefined => "Undefined".into(),
                ElfSymbolDefinition::Absolute => "Absolute".into(),
                ElfSymbolDefinition::Common => "Common".into(),
                ElfSymbolDefinition::Section(section_id) => section_name(object, section_id),
            },
            format!("{:#x}", symbol.value),
            format!("{:#x}", symbol.size),
        ])
    }
    vec![Box::new(table)]
}

fn render_section_strings(strings: &ElfStringTable) -> Vec<Box<dyn Widget>> {
    let mut table = Table::new();
    table.set_title("Strings table:");
    for (offset, string) in strings.all_with_offsets() {
        table.add_row([format!("{offset:#x}"), string.to_string()]);
    }
    vec![Box::new(table)]
}

fn render_section_relocs(
    object: &ElfObject<SerialIds>,
    relocs: &ElfRelocationsTable<SerialIds>,
) -> Vec<Box<dyn Widget>> {
    let intro = Text::new(format!(
        "symbol table:       {}\n\
         applies to section: {}",
        section_name(object, relocs.symbol_table),
        section_name(object, relocs.applies_to_section),
    ));

    let mut table = Table::new();
    table.set_title("Relocations:");
    table.add_row(["Type", "Symbol", "Offset", "Addend"]);
    for relocation in &relocs.relocations {
        let addend = match relocation.addend {
            Some(num @ 0..) => format!("{:#x}", num),
            Some(num) => format!("-{:#x}", num.abs()),
            None => "-".into(),
        };
        table.add_row([
            format!("{:?}", relocation.relocation_type),
            symbol_name(object, relocs.symbol_table, relocation.symbol),
            format!("{:#x}", relocation.offset),
            addend,
        ]);
    }

    vec![Box::new(intro), Box::new(table)]
}

fn render_section_notes(notes: &ElfNotesTable) -> Vec<Box<dyn Widget>> {
    let mut output = Vec::new();

    for note in &notes.notes {
        match note {
            ElfNote::Unknown(unknown) => output.push(Box::new(
                WidgetGroup::new()
                    .name(format!(
                        "unknown note with name {} and type {:#x}",
                        unknown.name, unknown.type_
                    ))
                    .add(HexDump::new(unknown.value.0.as_slice())),
            ) as Box<dyn Widget>),
        }
    }

    output
}

fn render_section_unknown(unknown: &ElfUnknownSection) -> Vec<Box<dyn Widget>> {
    vec![
        Box::new(Text::new(format!("unknown section with type {:#x}", unknown.id))),
        Box::new(HexDump::new(unknown.raw.0.as_slice())),
    ]
}