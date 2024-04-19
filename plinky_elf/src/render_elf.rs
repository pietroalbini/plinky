use crate::ids::serial::{SectionId, SerialIds, StringId, SymbolId};
use crate::ids::StringIdGetters;
use crate::{
    ElfABI, ElfClass, ElfDeduplication, ElfEndian, ElfMachine, ElfNote, ElfNotesTable, ElfObject,
    ElfPermissions, ElfProgramSection, ElfRelocationsTable, ElfSection, ElfSectionContent,
    ElfSegmentContent, ElfSegmentType, ElfStringTable, ElfSymbolBinding, ElfSymbolDefinition,
    ElfSymbolTable, ElfSymbolType, ElfType, ElfUninitializedSection, ElfUnknownSection,
};
use plinky_diagnostics::widgets::{HexDump, Table, Text, Widget, WidgetGroup};
use plinky_diagnostics::WidgetWriter;

pub fn render_elf(object: &ElfObject<SerialIds>) -> impl Widget {
    let mut widgets: Vec<Box<dyn Widget>> = Vec::new();
    widgets.push(Box::new(render_meta(object)));
    for (&id, section) in &object.sections {
        widgets.push(Box::new(render_section(object, id, section)));
    }
    widgets.push(Box::new(render_segments(object)));
    MultipleWidgets(widgets)
}

fn render_meta(object: &ElfObject<SerialIds>) -> impl Widget {
    let mut table = Table::new();
    table.set_title("Metadata:");
    table.add_row([
        "Class",
        match object.env.class {
            ElfClass::Elf32 => "ELF 32bit",
            ElfClass::Elf64 => "ELF 64bit",
        },
    ]);
    table.add_row([
        "Endian",
        match object.env.endian {
            ElfEndian::Little => "Little",
        },
    ]);
    table.add_row([
        "ABI",
        match object.env.abi {
            ElfABI::SystemV => "System V",
        },
    ]);
    table.add_row([
        "Machine",
        match object.env.machine {
            ElfMachine::X86 => "x86",
            ElfMachine::X86_64 => "x86-64",
        },
    ]);
    table.add_row([
        "Type",
        match object.type_ {
            ElfType::Relocatable => "Relocatable",
            ElfType::Executable => "Executable",
            ElfType::SharedObject => "Shared object",
            ElfType::Core => "Core dump",
        },
    ]);
    table.add_row([
        "Entrypoint".to_string(),
        match object.entry {
            Some(entry) => entry.to_string(),
            None => "-".to_string(),
        },
    ]);
    table
}

fn render_section(
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

fn render_segments(object: &ElfObject<SerialIds>) -> impl Widget {
    let mut table = Table::new();
    table.set_title("Segments:");
    table.add_row(["Type", "Perms", "Aligment", "Content"]);
    for segment in &object.segments {
        table.add_row([
            match segment.type_ {
                ElfSegmentType::Null => "Null".into(),
                ElfSegmentType::Load => "Load".into(),
                ElfSegmentType::Dynamic => "Dynamic".into(),
                ElfSegmentType::Interpreter => "Interpreter".into(),
                ElfSegmentType::Note => "Note".into(),
                ElfSegmentType::ProgramHeaderTable => "Program header table".into(),
                ElfSegmentType::GnuStack => "GNU stack".into(),
                ElfSegmentType::Unknown(id) => format!("<unknown: {id:#x}>"),
            },
            render_perms(&segment.perms),
            format!("{:#x}", segment.align),
            match &segment.content {
                ElfSegmentContent::Empty => "-".into(),
                ElfSegmentContent::Sections(sections) => sections
                    .iter()
                    .map(|&id| section_name(object, id))
                    .collect::<Vec<_>>()
                    .join(", "),
                ElfSegmentContent::Unknown(unknown) => format!(
                    "file: {:#x} (len: {:#x}), memory: {:#x} (len: {:#x})",
                    unknown.file_offset,
                    unknown.file_size,
                    unknown.virtual_address,
                    unknown.memory_size
                ),
            },
        ]);
    }
    table
}

fn render_perms(perms: &ElfPermissions) -> String {
    let mut output = String::new();
    let mut push = |cond: bool, chr: char| output.push(cond.then(|| chr).unwrap_or(' '));

    push(perms.read, 'R');
    push(perms.write, 'W');
    push(perms.execute, 'X');

    if output.trim().is_empty() {
        format!("{:1$}", "-", output.len())
    } else {
        output
    }
}

fn section_name(object: &ElfObject<SerialIds>, id: SectionId) -> String {
    let section = object.sections.get(&id).expect("invalid section id");
    format!("{}#{}", resolve_string(object, section.name), id.idx())
}

fn symbol_name(object: &ElfObject<SerialIds>, symbol_table_id: SectionId, id: SymbolId) -> String {
    let symbol_table = object.sections.get(&symbol_table_id).expect("invalid symbol table id");
    let ElfSectionContent::SymbolTable(symbol_table) = &symbol_table.content else {
        panic!("symbol table id is not a symbol table");
    };
    let symbol = symbol_table.symbols.get(&id).expect("invalid symbol id");
    format!("{}#{}#{}", resolve_string(object, symbol.name), symbol_table_id.idx(), id.idx())
}

fn resolve_string(object: &ElfObject<SerialIds>, id: StringId) -> &str {
    let table = object.sections.get(&id.section()).expect("invalid string section id");
    let ElfSectionContent::StringTable(table) = &table.content else {
        panic!("string section id is not a string table");
    };
    table.get(id.offset()).expect("missing string")
}

struct MultipleWidgets(Vec<Box<dyn Widget>>);

impl Widget for MultipleWidgets {
    fn render(&self, writer: &mut WidgetWriter<'_>) {
        for (i, widget) in self.0.iter().enumerate() {
            if i != 0 {
                writer.push_str("\n\n");
            }
            widget.render(writer);
        }
    }
}
