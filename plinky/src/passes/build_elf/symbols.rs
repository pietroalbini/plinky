use crate::passes::build_elf::ids::{BuiltElfIds, BuiltElfSectionId, BuiltElfSymbolId};
use crate::passes::build_elf::strings::BuiltStringsTable;
use crate::repr::sections::SymbolsSection;
use crate::repr::symbols::{Symbol, SymbolId, SymbolType, SymbolValue, SymbolVisibility, Symbols};
use plinky_elf::ids::serial::SectionId;
use plinky_elf::{
    ElfSectionContent, ElfSymbol, ElfSymbolBinding, ElfSymbolDefinition, ElfSymbolTable,
    ElfSymbolType, ElfSymbolVisibility,
};
use plinky_utils::ints::ExtractNumber;
use std::collections::BTreeMap;

pub(super) fn create_symbols(
    ids: &mut BuiltElfIds,
    section_ids: &BTreeMap<SectionId, BuiltElfSectionId>,
    string_tables: &BTreeMap<SectionId, BuiltStringsTable>,
    all_symbols: &Symbols,
    symbols_section: &SymbolsSection,
) -> BuiltSymbolsTable {
    let mut null_symbol = None;
    let mut global_symbols = Vec::new();
    let mut local_by_source = BTreeMap::new();
    for symbol in all_symbols.iter(&*symbols_section.view) {
        if symbol.id() == all_symbols.null_symbol_id() {
            assert!(null_symbol.is_none());
            null_symbol = Some(symbol);
        } else if let SymbolVisibility::Global { .. } = symbol.visibility() {
            global_symbols.push(symbol);
        } else {
            local_by_source.entry(symbol.stt_file()).or_insert_with(Vec::new).push(symbol);
        }
    }

    let mut converter = Converter {
        ids,
        section_ids,
        strings: &string_tables
            .get(&symbols_section.strings)
            .expect("missing string table for symbol table"),
        symbols: BTreeMap::new(),
        conversion: BTreeMap::new(),
    };

    converter.convert(null_symbol.expect("missing null symbol"));

    for (file, symbols_in_file) in local_by_source {
        if let Some(file) = file {
            converter.symbols.insert(
                converter.ids.allocate_symbol_id(),
                ElfSymbol {
                    name: *converter
                        .strings
                        .symbol_file_names
                        .get(&file)
                        .expect("no string for the file"),
                    binding: ElfSymbolBinding::Local,
                    type_: ElfSymbolType::File,
                    visibility: ElfSymbolVisibility::Default,
                    definition: ElfSymbolDefinition::Absolute,
                    value: 0,
                    size: 0,
                },
            );
        }
        for symbol in symbols_in_file {
            converter.convert(symbol);
        }
    }
    for symbol in global_symbols {
        converter.convert(symbol);
    }

    BuiltSymbolsTable {
        elf: Some(ElfSectionContent::SymbolTable(ElfSymbolTable {
            dynsym: symbols_section.is_dynamic,
            symbols: converter.symbols,
        })),
        conversion: converter.conversion,
    }
}

pub(super) struct BuiltSymbolsTable {
    pub(super) elf: Option<ElfSectionContent<BuiltElfIds>>,
    pub(super) conversion: BTreeMap<SymbolId, BuiltElfSymbolId>,
}

struct Converter<'a> {
    ids: &'a mut BuiltElfIds,
    section_ids: &'a BTreeMap<SectionId, BuiltElfSectionId>,
    strings: &'a BuiltStringsTable,
    symbols: BTreeMap<BuiltElfSymbolId, ElfSymbol<BuiltElfIds>>,
    conversion: BTreeMap<SymbolId, BuiltElfSymbolId>,
}

impl Converter<'_> {
    fn convert(&mut self, symbol: &Symbol) {
        let id = self.ids.allocate_symbol_id();
        self.symbols.insert(
            id,
            ElfSymbol {
                name: *self.strings.symbol_names.get(&symbol.id()).expect("no string for symbol"),
                binding: match symbol.visibility() {
                    SymbolVisibility::Local => ElfSymbolBinding::Local,
                    SymbolVisibility::Global { weak: true, hidden: _ } => ElfSymbolBinding::Weak,
                    SymbolVisibility::Global { weak: false, hidden: _ } => ElfSymbolBinding::Global,
                },
                visibility: match symbol.visibility() {
                    SymbolVisibility::Local => ElfSymbolVisibility::Default,
                    SymbolVisibility::Global { weak: _, hidden: false } => {
                        ElfSymbolVisibility::Default
                    }
                    SymbolVisibility::Global { weak: _, hidden: true } => {
                        ElfSymbolVisibility::Hidden
                    }
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
                        ElfSymbolDefinition::Section(*self.section_ids.get(&section).unwrap())
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
        self.conversion.insert(symbol.id(), id);
    }
}
