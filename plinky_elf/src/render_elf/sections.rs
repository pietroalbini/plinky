use crate::ids::ElfIds;
use crate::render_elf::utils::{render_perms, section_name, symbol_name};
use crate::{
    ElfDeduplication, ElfGroup, ElfHash, ElfNote, ElfNotesTable, ElfObject, ElfProgramSection,
    ElfRelocationsTable, ElfSection, ElfSectionContent, ElfStringTable, ElfSymbolBinding,
    ElfSymbolDefinition, ElfSymbolTable, ElfSymbolType, ElfSymbolVisibility,
    ElfUninitializedSection, ElfUnknownSection,
};
use plinky_diagnostics::widgets::{HexDump, Table, Text, Widget, WidgetGroup};
use std::collections::BTreeMap;

pub(super) fn render_section<I: ElfIds>(
    object: &ElfObject<I>,
    id: &I::SectionId,
    section: &ElfSection<I>,
) -> impl Widget {
    let content: Vec<Box<dyn Widget>> = match &section.content {
        ElfSectionContent::Null => vec![Box::new(Text::new("empty section"))],
        ElfSectionContent::Program(program) => render_section_program(program),
        ElfSectionContent::Uninitialized(uninit) => render_section_uninit(uninit),
        ElfSectionContent::SymbolTable(symbols) => render_section_symbols(object, id, symbols),
        ElfSectionContent::StringTable(strings) => render_section_strings(strings),
        ElfSectionContent::RelocationsTable(relocs) => render_section_relocs(object, relocs),
        ElfSectionContent::Group(group) => render_section_group(object, group),
        ElfSectionContent::Hash(hash) => render_section_hash(object, hash),
        ElfSectionContent::Note(notes) => render_section_notes(notes),
        ElfSectionContent::Unknown(unknown) => render_section_unknown(unknown),
    };

    let mut extra = String::new();
    if section.part_of_group {
        extra.push_str(", part of a group");
    }

    WidgetGroup::new()
        .name(format!(
            "section {} (address: {:#x}{extra})",
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

fn render_section_symbols<I: ElfIds>(
    object: &ElfObject<I>,
    section_id: &I::SectionId,
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
            symbol_name(object, section_id, id),
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

fn render_section_relocs<I: ElfIds>(
    object: &ElfObject<I>,
    relocs: &ElfRelocationsTable<I>,
) -> Vec<Box<dyn Widget>> {
    let intro = Text::new(format!(
        "symbol table:       {}\n\
         applies to section: {}",
        section_name(object, &relocs.symbol_table),
        section_name(object, &relocs.applies_to_section),
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
            symbol_name(object, &relocs.symbol_table, &relocation.symbol),
            format!("{:#x}", relocation.offset),
            addend,
        ]);
    }

    vec![Box::new(intro), Box::new(table)]
}

fn render_section_group<I: ElfIds>(
    object: &ElfObject<I>,
    group: &ElfGroup<I>,
) -> Vec<Box<dyn Widget>> {
    let mut info = "group | ".to_string();
    if group.comdat {
        info.push_str("COMDAT | ");
    }
    info.push_str("signature: ");
    info.push_str(&symbol_name(object, &group.symbol_table, &group.signature));

    let mut sections = Table::new();
    sections.set_title("Sections:");
    for section in &group.sections {
        sections.add_row([section_name(object, section)]);
    }

    vec![Box::new(Text::new(info)), Box::new(sections)]
}

fn render_section_hash<I: ElfIds>(
    object: &ElfObject<I>,
    hash: &ElfHash<I>,
) -> Vec<Box<dyn Widget>> {
    let info = Text::new(format!("Hash table for {}", section_name(object, &hash.symbol_table)));

    let mut buckets_with_count = BTreeMap::new();
    for mut entry in hash.buckets.iter().copied() {
        let mut count = 0;
        while entry != 0 {
            count += 1;
            entry = hash.chain[entry as usize];
        }
        *buckets_with_count.entry(count).or_insert(0) += 1;
    }

    let mut stats = Table::new();
    stats.set_title("Statistics:");
    stats.add_row(["Number of entries in the bucket", "Number of buckets with those entries"]);
    for (entries, buckets) in buckets_with_count {
        stats.add_row([entries.to_string(), buckets.to_string()]);
    }

    vec![Box::new(info), Box::new(stats)]
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
