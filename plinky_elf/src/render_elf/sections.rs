use crate::ids::ElfIds;
use crate::render_elf::names::Names;
use crate::render_elf::utils::render_perms;
use crate::{
    ElfDeduplication, ElfDynamic, ElfDynamicDirective, ElfGroup, ElfHash, ElfNote, ElfNotesTable,
    ElfObject, ElfPLTRelocationsMode, ElfProgramSection, ElfRelocationsTable, ElfSection,
    ElfSectionContent, ElfStringTable, ElfSymbolBinding, ElfSymbolDefinition, ElfSymbolTable,
    ElfSymbolType, ElfSymbolVisibility, ElfUninitializedSection, ElfUnknownSection,
};
use plinky_diagnostics::widgets::{HexDump, Table, Text, Widget, WidgetGroup};

pub(super) fn render_section<I: ElfIds>(
    names: &Names<I>,
    object: &ElfObject<I>,
    id: &I::SectionId,
    section: &ElfSection<I>,
) -> impl Widget {
    let content: Vec<Box<dyn Widget>> = match &section.content {
        ElfSectionContent::Null => vec![Box::new(Text::new("empty section"))],
        ElfSectionContent::Program(program) => render_section_program(program),
        ElfSectionContent::Uninitialized(uninit) => render_section_uninit(uninit),
        ElfSectionContent::SymbolTable(symbols) => render_section_symbols(names, symbols),
        ElfSectionContent::StringTable(strings) => render_section_strings(strings),
        ElfSectionContent::RelocationsTable(relocs) => render_section_relocs(names, relocs),
        ElfSectionContent::Group(group) => render_section_group(names, group),
        ElfSectionContent::Hash(hash) => render_section_hash(names, object, hash),
        ElfSectionContent::Note(notes) => render_section_notes(notes),
        ElfSectionContent::Dynamic(dynamic) => render_section_dynamic(names, dynamic),
        ElfSectionContent::Unknown(unknown) => render_section_unknown(unknown),
    };

    let mut extra = String::new();
    if section.part_of_group {
        extra.push_str(", part of a group");
    }

    WidgetGroup::new()
        .name(format!(
            "section {} (address: {:#x}{extra})",
            names.section(id),
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

    vec![Box::new(Text::new(intro.trim())), Box::new(HexDump::new(program.raw.as_slice()))]
}

fn render_section_uninit(uninit: &ElfUninitializedSection) -> Vec<Box<dyn Widget>> {
    vec![Box::new(Text::new(format!(
        "uninitialized | len: {:#x} | permissions: {}",
        uninit.len,
        render_perms(&uninit.perms)
    )))]
}

fn render_section_symbols<I: ElfIds>(
    names: &Names<I>,
    symbols: &ElfSymbolTable<I>,
) -> Vec<Box<dyn Widget>> {
    let mut table = Table::new();
    if symbols.dynsym {
        table.set_title("Dynamic symbol table:");
    } else {
        table.set_title("Symbol table:");
    }
    table.add_row(["Name", "Binding", "Type", "Visibility", "Definition", "Value", "Size"]);
    for (id, symbol) in &symbols.symbols {
        table.add_row([
            names.symbol(id).to_string(),
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
            match symbol.visibility {
                ElfSymbolVisibility::Default => "Default",
                ElfSymbolVisibility::Hidden => "Hidden",
                ElfSymbolVisibility::Protected => "Protected",
                ElfSymbolVisibility::Exported => "Exported",
                ElfSymbolVisibility::Singleton => "Singleton",
                ElfSymbolVisibility::Eliminate => "Eliminate",
            }
            .into(),
            match &symbol.definition {
                ElfSymbolDefinition::Undefined => "Undefined".into(),
                ElfSymbolDefinition::Absolute => "Absolute".into(),
                ElfSymbolDefinition::Common => "Common".into(),
                ElfSymbolDefinition::Section(section_id) => names.section(section_id).into(),
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

fn render_section_relocs<I: ElfIds>(
    names: &Names<I>,
    relocs: &ElfRelocationsTable<I>,
) -> Vec<Box<dyn Widget>> {
    let intro = Text::new(format!(
        "symbol table:       {}\n\
         applies to section: {}",
        names.section(&relocs.symbol_table),
        names.section(&relocs.applies_to_section),
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
            names.symbol(&relocation.symbol).to_string(),
            format!("{:#x}", relocation.offset),
            addend,
        ]);
    }

    vec![Box::new(intro), Box::new(table)]
}

fn render_section_group<I: ElfIds>(names: &Names<I>, group: &ElfGroup<I>) -> Vec<Box<dyn Widget>> {
    let mut info = "group | ".to_string();
    if group.comdat {
        info.push_str("COMDAT | ");
    }
    info.push_str("signature: ");
    info.push_str(names.symbol(&group.signature));

    let mut sections = Table::new();
    sections.set_title("Sections:");
    for section in &group.sections {
        sections.add_row([names.section(section)]);
    }

    vec![Box::new(Text::new(info)), Box::new(sections)]
}

fn render_section_hash<I: ElfIds>(
    names: &Names<I>,
    object: &ElfObject<I>,
    hash: &ElfHash<I>,
) -> Vec<Box<dyn Widget>> {
    let ElfSectionContent::SymbolTable(symbol_table) =
        &object.sections.get(&hash.symbol_table).unwrap().content
    else {
        panic!("hash table's symbol table is not a symbol table");
    };

    let info = Text::new(format!("Hash table for {}", names.section(&hash.symbol_table)));

    let mut buckets = Vec::new();
    for mut entry in hash.buckets.iter().copied() {
        let mut items = Vec::new();
        while entry != 0 {
            items.push(symbol_table.symbols.keys().skip(entry as usize).next().unwrap());
            entry = hash.chain[entry as usize];
        }
        buckets.push(items);
    }

    let mut content = Table::new();
    content.set_title("Content:");
    content.add_row(["Bucket ID", "Symbols in bucket"]);
    for (id, symbols) in buckets.iter().enumerate() {
        let mut symbols_str = String::new();
        for (pos, symbol) in symbols.iter().enumerate() {
            if pos != 0 {
                symbols_str.push('\n');
            }
            symbols_str.push_str(names.symbol(symbol));
        }
        content.add_row([id.to_string(), symbols_str]);
    }

    vec![Box::new(info), Box::new(content)]
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
                    .add(HexDump::new(unknown.value.as_slice())),
            ) as Box<dyn Widget>),
        }
    }

    output
}

fn render_section_dynamic<I: ElfIds>(
    names: &Names<I>,
    dynamic: &ElfDynamic<I>,
) -> Vec<Box<dyn Widget>> {
    enum Value<'a> {
        Bytes(&'a u64),
        Addr(&'a u64),
        StrOff(&'a u64),
        Str(String),
        None,
    }

    let info = Text::new(format!(
        "dynamic information | string table: {}",
        names.section(&dynamic.string_table),
    ));

    let mut table = Table::new();
    table.add_row(["Kind", "Value"]);
    for directive in &dynamic.directives {
        let (name, value) = match directive {
            ElfDynamicDirective::Null => ("Null", Value::None),
            ElfDynamicDirective::Needed { string_table_offset } => {
                ("Needed libraries", Value::StrOff(string_table_offset))
            }
            ElfDynamicDirective::PLTRelocationsSize { bytes } => {
                ("PLT relocations size", Value::Bytes(bytes))
            }
            ElfDynamicDirective::PLTGOT { address } => ("PLT GOT", Value::Addr(address)),
            ElfDynamicDirective::Hash { address } => ("Hash table", Value::Addr(address)),
            ElfDynamicDirective::GnuHash { address } => ("GNU hash table", Value::Addr(address)),
            ElfDynamicDirective::StringTable { address } => ("String table", Value::Addr(address)),
            ElfDynamicDirective::SymbolTable { address } => ("Symbol table", Value::Addr(address)),
            ElfDynamicDirective::Rela { address } => ("RelocationsA table", Value::Addr(address)),
            ElfDynamicDirective::RelaSize { bytes } => ("RelocationsA size", Value::Bytes(bytes)),
            ElfDynamicDirective::RelaEntrySize { bytes } => {
                ("RelocationsA entry size", Value::Bytes(bytes))
            }
            ElfDynamicDirective::StringTableSize { bytes } => {
                ("String table size", Value::Bytes(bytes))
            }
            ElfDynamicDirective::SymbolTableEntrySize { bytes } => {
                ("Symbol table entry size", Value::Bytes(bytes))
            }
            ElfDynamicDirective::InitFunction { address } => {
                ("Initialization function", Value::Addr(address))
            }
            ElfDynamicDirective::FiniFunction { address } => {
                ("Finalization function", Value::Addr(address))
            }
            ElfDynamicDirective::SharedObjectName { string_table_offset } => {
                ("Shared object name", Value::StrOff(string_table_offset))
            }
            ElfDynamicDirective::RuntimePath { string_table_offset } => {
                ("Runtime path", Value::StrOff(string_table_offset))
            }
            ElfDynamicDirective::Symbolic => ("Enable symbolic resolution", Value::None),
            ElfDynamicDirective::Rel { address } => ("Relocations table", Value::Addr(address)),
            ElfDynamicDirective::RelSize { bytes } => {
                ("Relocations table size", Value::Bytes(bytes))
            }
            ElfDynamicDirective::RelEntrySize { bytes } => {
                ("Relocations table entry size", Value::Bytes(bytes))
            }
            ElfDynamicDirective::PTLRelocationsMode { mode } => (
                "PLT relocations type",
                Value::Str(match mode {
                    ElfPLTRelocationsMode::Rel => "Relocations".into(),
                    ElfPLTRelocationsMode::Rela => "RelocationsA".into(),
                    ElfPLTRelocationsMode::Unknown(num) => format!("<unknown {num:#x}>"),
                }),
            ),
            ElfDynamicDirective::Debug { address } => ("Debug information", Value::Addr(address)),
            ElfDynamicDirective::RelocationsWillModifyText => {
                ("Relocations will modify text sections", Value::None)
            }
            ElfDynamicDirective::JumpRel { address } => {
                ("Jump PLT relocations", Value::Addr(address))
            }
            ElfDynamicDirective::BindNow => ("Bind now", Value::None),
            ElfDynamicDirective::Flags(flags) => ("Flags", Value::Str(flags.to_string())),
            ElfDynamicDirective::Flags1(flags1) => ("Flags1", Value::Str(flags1.to_string())),
            ElfDynamicDirective::Unknown { tag, value } => {
                table.add_row([format!("<unknown {tag:#x}>"), format!("{value:#x}")]);
                continue;
            }
        };
        table.add_row([
            name.into(),
            match value {
                Value::Bytes(bytes) => format!("{bytes} bytes"),
                Value::Addr(addr) => format!("address {addr:#x}"),
                Value::StrOff(off) => format!("offset {off:#} in the string table"),
                Value::Str(string) => string,
                Value::None => "-".to_string(),
            },
        ]);
    }

    vec![Box::new(info), Box::new(table)]
}

fn render_section_unknown(unknown: &ElfUnknownSection) -> Vec<Box<dyn Widget>> {
    vec![
        Box::new(Text::new(format!("unknown section with type {:#x}", unknown.id))),
        Box::new(HexDump::new(unknown.raw.as_slice())),
    ]
}
