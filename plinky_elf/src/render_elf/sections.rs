use crate::ids::ElfSectionId;
use crate::render_elf::names::Names;
use crate::render_elf::utils::render_perms;
use crate::{
    ElfClass, ElfDeduplication, ElfDynamic, ElfDynamicDirective, ElfGnuHash, ElfGnuProperty,
    ElfGroup, ElfHash, ElfNote, ElfNotesTable, ElfObject, ElfPLTRelocationsMode, ElfProgramSection,
    ElfRelTable, ElfRelaTable, ElfSection, ElfSectionContent, ElfStringTable, ElfSymbolDefinition,
    ElfSymbolTable, ElfSymbolType, ElfUninitializedSection, ElfUnknownSection,
};
use plinky_diagnostics::widgets::{HexDump, Table, Text, Widget, WidgetGroup};

pub(super) fn render_section(
    names: &Names,
    object: &ElfObject,
    id: ElfSectionId,
    section: &ElfSection,
) -> impl Widget + use<> {
    let content: Vec<Box<dyn Widget>> = match &section.content {
        ElfSectionContent::Null => vec![Box::new(Text::new("empty section"))],
        ElfSectionContent::Program(program) => render_section_program(program),
        ElfSectionContent::Uninitialized(uninit) => render_section_uninit(uninit),
        ElfSectionContent::SymbolTable(symbols) => render_section_symbols(names, symbols),
        ElfSectionContent::StringTable(strings) => render_section_strings(strings),
        ElfSectionContent::Rel(rel) => render_section_rel(names, rel),
        ElfSectionContent::Rela(rela) => render_section_rela(names, rela),
        ElfSectionContent::Group(group) => render_section_group(names, group),
        ElfSectionContent::Hash(hash) => render_section_hash(names, object, hash),
        ElfSectionContent::GnuHash(gnu_hash) => render_section_gnu_hash(names, object, gnu_hash),
        ElfSectionContent::Note(notes) => render_section_notes(notes),
        ElfSectionContent::Dynamic(dynamic) => render_section_dynamic(names, object, dynamic),
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

fn render_section_symbols(names: &Names, symbols: &ElfSymbolTable) -> Vec<Box<dyn Widget>> {
    let mut table = Table::new();
    if symbols.dynsym {
        table.set_title("Dynamic symbol table:");
    } else {
        table.set_title("Symbol table:");
    }
    table.add_head(["Name", "Binding", "Type", "Visibility", "Definition", "Value", "Size"]);
    for (id, symbol) in &symbols.symbols {
        table.add_body([
            names.symbol(*id).to_string(),
            symbol.binding.to_string(),
            match symbol.type_ {
                ElfSymbolType::NoType => "-".into(),
                ElfSymbolType::Object => "Object".into(),
                ElfSymbolType::Function => "Function".into(),
                ElfSymbolType::Section => "Section".into(),
                ElfSymbolType::File => "File".into(),
                ElfSymbolType::Unknown(unknown) => format!("<unknown: {unknown:#x}>"),
            },
            symbol.visibility.to_string(),
            match &symbol.definition {
                ElfSymbolDefinition::Undefined => "Undefined".into(),
                ElfSymbolDefinition::Absolute => "Absolute".into(),
                ElfSymbolDefinition::Common => "Common".into(),
                ElfSymbolDefinition::Section(section_id) => names.section(*section_id).into(),
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
        table.add_body([format!("{offset:#x}"), string.to_string()]);
    }
    vec![Box::new(table)]
}

fn render_section_rel(names: &Names, rel: &ElfRelTable) -> Vec<Box<dyn Widget>> {
    let intro = Text::new(format!(
        "symbol table:       {}\n\
         applies to section: {}",
        names.section(rel.symbol_table),
        names.section(rel.applies_to_section),
    ));

    let mut table = Table::new();
    table.set_title("Relocations:");
    table.add_head(["Type", "Symbol", "Offset"]);
    for relocation in &rel.relocations {
        table.add_body([
            format!("{:?}", relocation.relocation_type),
            names.symbol(relocation.symbol).to_string(),
            format!("{:#x}", relocation.offset),
        ]);
    }

    vec![Box::new(intro), Box::new(table)]
}

fn render_section_rela(names: &Names, rela: &ElfRelaTable) -> Vec<Box<dyn Widget>> {
    let intro = Text::new(format!(
        "symbol table:       {}\n\
         applies to section: {}",
        names.section(rela.symbol_table),
        names.section(rela.applies_to_section),
    ));

    let mut table = Table::new();
    table.set_title("Relocations:");
    table.add_head(["Type", "Symbol", "Offset", "Addend"]);
    for relocation in &rela.relocations {
        let addend = if relocation.addend >= 0 {
            format!("{:#x}", relocation.addend)
        } else {
            format!("-{:#x}", relocation.addend.abs())
        };
        table.add_body([
            format!("{:?}", relocation.relocation_type),
            names.symbol(relocation.symbol).to_string(),
            format!("{:#x}", relocation.offset),
            addend,
        ]);
    }

    vec![Box::new(intro), Box::new(table)]
}

fn render_section_group(names: &Names, group: &ElfGroup) -> Vec<Box<dyn Widget>> {
    let mut info = "group | ".to_string();
    if group.comdat {
        info.push_str("COMDAT | ");
    }
    info.push_str("signature: ");
    info.push_str(names.symbol(group.signature));

    let mut sections = Table::new();
    sections.set_title("Sections:");
    for section in &group.sections {
        sections.add_body([names.section(*section)]);
    }

    vec![Box::new(Text::new(info)), Box::new(sections)]
}

fn render_section_hash(names: &Names, object: &ElfObject, hash: &ElfHash) -> Vec<Box<dyn Widget>> {
    let ElfSectionContent::SymbolTable(symbol_table) =
        &object.sections.get(&hash.symbol_table).unwrap().content
    else {
        panic!("hash table's symbol table is not a symbol table");
    };

    let info = Text::new(format!("Hash table for {}", names.section(hash.symbol_table)));

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
    content.add_head(["Bucket ID", "Symbols in bucket"]);
    for (id, symbols) in buckets.iter().enumerate() {
        let mut symbols_str = String::new();
        for (pos, symbol) in symbols.iter().enumerate() {
            if pos != 0 {
                symbols_str.push('\n');
            }
            symbols_str.push_str(names.symbol(**symbol));
        }
        content.add_body([id.to_string(), symbols_str]);
    }

    vec![Box::new(info), Box::new(content)]
}

fn render_section_gnu_hash(
    names: &Names,
    object: &ElfObject,
    gnu_hash: &ElfGnuHash,
) -> Vec<Box<dyn Widget>> {
    let ElfSectionContent::SymbolTable(symbol_table) =
        &object.sections.get(&gnu_hash.symbol_table).unwrap().content
    else {
        panic!("hash table's symbol table is not a symbol table");
    };

    let info = Text::new(format!(
        "GNU hash table for {}\nIgnored symbols: {}",
        names.section(gnu_hash.symbol_table),
        gnu_hash.symbols_offset,
    ));

    let bloom_bits = match object.env.class {
        ElfClass::Elf32 => 32,
        ElfClass::Elf64 => 64,
    };
    let mut bloom = Table::new();
    bloom.set_title(format!("Bloom filter (shift of {}):", gnu_hash.bloom_shift));
    for byte in &gnu_hash.bloom {
        bloom.add_body([format!("{byte:0>bloom_bits$b}")]);
    }

    let mut buckets = Vec::new();
    for start_symtab_index in &gnu_hash.buckets {
        let mut symbols = Vec::new();
        for (idx, hash) in gnu_hash
            .chain
            .iter()
            .enumerate()
            .skip((*start_symtab_index - gnu_hash.symbols_offset) as usize)
        {
            symbols.push((
                symbol_table
                    .symbols
                    .keys()
                    .skip(idx + gnu_hash.symbols_offset as usize)
                    .next()
                    .unwrap(),
                hash & 0xfffffffe,
            ));

            // The chain ends when the least significant bit of the hash is 1.
            if (hash & 1) == 1 {
                break;
            }
        }
        buckets.push(symbols);
    }

    let mut content = Table::new();
    content.set_title("Content:");
    content.add_head(["Bucket ID", "Symbols in bucket", "Truncated hashes"]);
    for (id, symbols) in buckets.iter().enumerate() {
        let mut symbols_str = String::new();
        let mut hashes_str = String::new();
        for (pos, (symbol, hash)) in symbols.iter().enumerate() {
            if pos != 0 {
                symbols_str.push('\n');
                hashes_str.push('\n');
            }
            symbols_str.push_str(names.symbol(**symbol));
            hashes_str.push_str(&format!("{hash:0>8x}"));
        }
        content.add_body([id.to_string(), symbols_str, hashes_str]);
    }

    vec![Box::new(info), Box::new(bloom), Box::new(content)]
}

fn render_section_notes(notes: &ElfNotesTable) -> Vec<Box<dyn Widget>> {
    notes.notes.iter().map(render_note).collect()
}

pub fn render_note(note: &ElfNote) -> Box<dyn Widget> {
    match note {
        ElfNote::Unknown(unknown) => Box::new(
            WidgetGroup::new()
                .name(format!(
                    "unknown note with name {} and type {:#x}",
                    unknown.name, unknown.type_
                ))
                .add(HexDump::new(unknown.value.as_slice())),
        ),
        ElfNote::GnuProperties(properties) => {
            let mut table = Table::new();
            table.add_head(["Property", "Value"]);
            for property in properties {
                match property {
                    ElfGnuProperty::X86Features2Used(features2) => {
                        table.add_body(["x86 features (2) used".into(), features2.to_string()]);
                    }
                    ElfGnuProperty::X86IsaUsed(isa) => {
                        table.add_body(["x86 ISA used".into(), isa.to_string()]);
                    }
                    ElfGnuProperty::Unknown(unknown) => {
                        table.add_body([
                            format!("Unknown (type {:#x})", unknown.type_),
                            format!("{:?}", unknown.data),
                        ]);
                    }
                }
            }
            Box::new(WidgetGroup::new().name("GNU properties").add(table))
        }
    }
}

fn render_section_dynamic(
    names: &Names,
    object: &ElfObject,
    dynamic: &ElfDynamic,
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
        names.section(dynamic.string_table),
    ));

    let string_table = object.sections.get(&dynamic.string_table).expect("missing string table");
    let ElfSectionContent::StringTable(strings) = &string_table.content else {
        panic!("the dynamic section's string table is not a string table");
    };

    let mut table = Table::new();
    table.add_head(["Kind", "Value"]);
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
                table.add_body([format!("<unknown {tag:#x}>"), format!("{value:#x}")]);
                continue;
            }
        };
        table.add_body([
            name.into(),
            match value {
                Value::Bytes(bytes) => format!("{bytes} bytes"),
                Value::Addr(addr) => format!("address {addr:#x}"),
                Value::StrOff(off) => {
                    let string = strings.get(*off as _).unwrap_or("<missing>");
                    format!("string {off:#x}: {string}")
                }
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
