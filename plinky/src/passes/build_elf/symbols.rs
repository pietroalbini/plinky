use crate::passes::build_elf::ids::{BuiltElfIds, BuiltElfSectionId, BuiltElfSymbolId};
use crate::passes::build_elf::sections::Sections;
use crate::passes::build_elf::PendingStringsTable;
use crate::repr::symbols::views::SymbolsView;
use crate::repr::symbols::{Symbol, SymbolType, SymbolValue, SymbolVisibility, Symbols};
use plinky_utils::ints::ExtractNumber;
use plinky_elf::ids::serial::SymbolId;
use plinky_elf::{
    ElfSectionContent, ElfSymbol, ElfSymbolBinding, ElfSymbolDefinition, ElfSymbolTable,
    ElfSymbolType, ElfSymbolVisibility,
};
use std::collections::BTreeMap;

pub(super) fn create_symbols<'a>(
    all_symbols: &Symbols,
    view: &dyn SymbolsView,
    ids: &mut BuiltElfIds,
    sections: &mut Sections,
    string_table_id: BuiltElfSectionId,
    is_dynamic: bool,
) -> CreateSymbolsOutput {
    let mut strings = PendingStringsTable::new(string_table_id);
    let mut symbols = BTreeMap::new();
    let mut conversion = BTreeMap::new();

    let mut null_symbol = None;
    let mut global_symbols = Vec::new();
    let mut local_by_source = BTreeMap::new();
    for (symbol_id, symbol) in all_symbols.iter(view) {
        if symbol_id == all_symbols.null_symbol_id() {
            assert!(null_symbol.is_none());
            null_symbol = Some(symbol);
        } else if let SymbolVisibility::Global { .. } = symbol.visibility() {
            global_symbols.push(symbol);
        } else {
            local_by_source.entry(symbol.stt_file()).or_insert_with(Vec::new).push(symbol);
        }
    }

    add_symbol(
        ids,
        sections,
        &mut symbols,
        &mut strings,
        &mut conversion,
        null_symbol.expect("missing null symbol"),
    );

    for (file, symbols_in_file) in local_by_source {
        symbols.insert(
            ids.allocate_symbol_id(),
            ElfSymbol {
                name: strings.add(file.expect("symbol without a STT_FILE").resolve().as_str()),
                binding: ElfSymbolBinding::Local,
                type_: ElfSymbolType::File,
                visibility: ElfSymbolVisibility::Default,
                definition: ElfSymbolDefinition::Absolute,
                value: 0,
                size: 0,
            },
        );
        for symbol in symbols_in_file {
            add_symbol(ids, sections, &mut symbols, &mut strings, &mut conversion, symbol);
        }
    }
    for symbol in global_symbols {
        add_symbol(ids, sections, &mut symbols, &mut strings, &mut conversion, symbol);
    }

    CreateSymbolsOutput {
        symbol_table: ElfSectionContent::SymbolTable(ElfSymbolTable {
            dynsym: is_dynamic,
            symbols,
        }),
        string_table: strings.into_elf(),
        conversion,
    }
}

pub(super) struct CreateSymbolsOutput {
    pub(super) symbol_table: ElfSectionContent<BuiltElfIds>,
    pub(super) string_table: ElfSectionContent<BuiltElfIds>,
    pub(super) conversion: BTreeMap<SymbolId, BuiltElfSymbolId>,
}

fn add_symbol(
    ids: &mut BuiltElfIds,
    sections: &mut Sections,
    symbols: &mut BTreeMap<BuiltElfSymbolId, ElfSymbol<BuiltElfIds>>,
    strings: &mut PendingStringsTable,
    conversion: &mut BTreeMap<SymbolId, BuiltElfSymbolId>,
    symbol: &Symbol,
) {
    let id = ids.allocate_symbol_id();
    symbols.insert(
        id,
        ElfSymbol {
            name: strings.add(symbol.name().resolve().as_str()),
            binding: match symbol.visibility() {
                SymbolVisibility::Local => ElfSymbolBinding::Local,
                SymbolVisibility::Global { weak: true, hidden: _ } => ElfSymbolBinding::Weak,
                SymbolVisibility::Global { weak: false, hidden: _ } => ElfSymbolBinding::Global,
            },
            visibility: match symbol.visibility() {
                SymbolVisibility::Local => ElfSymbolVisibility::Default,
                SymbolVisibility::Global { weak: _, hidden: false } => ElfSymbolVisibility::Default,
                SymbolVisibility::Global { weak: _, hidden: true } => ElfSymbolVisibility::Hidden,
            },
            type_: match symbol.type_() {
                SymbolType::NoType => ElfSymbolType::NoType,
                SymbolType::Function => ElfSymbolType::Function,
                SymbolType::Object => ElfSymbolType::Object,
                SymbolType::Section => ElfSymbolType::Section,
            },
            definition: match symbol.value() {
                SymbolValue::Absolute { .. } => ElfSymbolDefinition::Absolute,
                SymbolValue::SectionRelative { .. } => {
                    panic!("section relative addresses should not reach this stage");
                }
                SymbolValue::SectionVirtualAddress { section, .. } => {
                    ElfSymbolDefinition::Section(sections.new_id_of(section))
                }
                SymbolValue::Undefined => ElfSymbolDefinition::Undefined,
                SymbolValue::Null => ElfSymbolDefinition::Undefined,
            },
            value: match symbol.value() {
                SymbolValue::Absolute { value } => value.extract(),
                SymbolValue::SectionRelative { .. } => {
                    panic!("section relative addresses should not reach this stage");
                }
                SymbolValue::SectionVirtualAddress { memory_address, .. } => {
                    memory_address.extract()
                }
                SymbolValue::Undefined => 0,
                SymbolValue::Null => 0,
            },
            size: 0,
        },
    );
    conversion.insert(symbol.id(), id);
}
