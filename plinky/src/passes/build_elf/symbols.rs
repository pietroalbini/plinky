use crate::passes::build_elf::ids::{BuiltElfIds, BuiltElfSectionId, BuiltElfSymbolId};
use crate::passes::build_elf::sections::Sections;
use crate::passes::build_elf::{ElfBuilder, PendingStringsTable};
use crate::repr::symbols::{Symbol, SymbolType, SymbolValue, SymbolVisibility, Symbols};
use crate::utils::ints::ExtractNumber;
use plinky_elf::ids::serial::SymbolId;
use plinky_elf::{
    ElfSectionContent, ElfSymbol, ElfSymbolBinding, ElfSymbolDefinition, ElfSymbolTable,
    ElfSymbolType, ElfSymbolVisibility,
};
use std::collections::BTreeMap;

pub(super) fn add_symbols<'a, F, I>(
    builder: &'a mut ElfBuilder<'_>,
    symtab: &str,
    strtab: &str,
    is_dynamic: bool,
    getter: F,
) -> AddSymbolsOutput
where
    F: FnOnce(&'a Symbols) -> I,
    I: Iterator<Item = (SymbolId, &'a Symbol)>,
{
    let mut strings = PendingStringsTable::new(&mut builder.ids);
    let strings_id = strings.id;
    let mut symbols = BTreeMap::new();
    let mut conversion = BTreeMap::new();

    let mut null_symbol = None;
    let mut global_symbols = Vec::new();
    let mut local_by_source = BTreeMap::new();
    for (symbol_id, symbol) in getter(&builder.object.symbols) {
        if symbol_id == builder.object.symbols.null_symbol_id() {
            assert!(null_symbol.is_none());
            null_symbol = Some(symbol);
        } else if let SymbolVisibility::Global { .. } = &symbol.visibility {
            global_symbols.push(symbol);
        } else {
            local_by_source.entry(symbol.stt_file).or_insert_with(Vec::new).push(symbol);
        }
    }

    add_symbol(
        &mut builder.ids,
        &mut builder.sections,
        &mut symbols,
        &mut strings,
        &mut conversion,
        null_symbol.expect("missing null symbol"),
    );

    for (file, symbols_in_file) in local_by_source {
        symbols.insert(
            builder.ids.allocate_symbol_id(),
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
            add_symbol(
                &mut builder.ids,
                &mut builder.sections,
                &mut symbols,
                &mut strings,
                &mut conversion,
                symbol,
            );
        }
    }
    for symbol in global_symbols {
        add_symbol(
            &mut builder.ids,
            &mut builder.sections,
            &mut symbols,
            &mut strings,
            &mut conversion,
            symbol,
        );
    }

    let table = builder
        .sections
        .create(
            symtab,
            ElfSectionContent::SymbolTable(ElfSymbolTable { dynsym: is_dynamic, symbols }),
        )
        .add(&mut builder.ids);
    builder.sections.create(strtab, strings.into_elf()).add_with_id(strings_id);

    AddSymbolsOutput { table, conversion }
}

pub(super) struct AddSymbolsOutput {
    pub(super) table: BuiltElfSectionId,
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
            name: strings.add(symbol.name.resolve().as_str()),
            binding: match &symbol.visibility {
                SymbolVisibility::Local => ElfSymbolBinding::Local,
                SymbolVisibility::Global { weak: true, hidden: _ } => ElfSymbolBinding::Weak,
                SymbolVisibility::Global { weak: false, hidden: _ } => ElfSymbolBinding::Global,
            },
            visibility: match &symbol.visibility {
                SymbolVisibility::Local => ElfSymbolVisibility::Default,
                SymbolVisibility::Global { weak: _, hidden: false } => ElfSymbolVisibility::Default,
                SymbolVisibility::Global { weak: _, hidden: true } => ElfSymbolVisibility::Hidden,
            },
            type_: match &symbol.type_ {
                SymbolType::NoType => ElfSymbolType::NoType,
                SymbolType::Function => ElfSymbolType::Function,
                SymbolType::Object => ElfSymbolType::Object,
                SymbolType::Section => ElfSymbolType::Section,
            },
            definition: match &symbol.value {
                SymbolValue::Absolute { .. } => ElfSymbolDefinition::Absolute,
                SymbolValue::SectionRelative { .. } => {
                    panic!("section relative addresses should not reach this stage");
                }
                SymbolValue::SectionVirtualAddress { section, .. } => {
                    ElfSymbolDefinition::Section(sections.new_id_of(*section))
                }
                SymbolValue::Undefined => ElfSymbolDefinition::Undefined,
                SymbolValue::Null => ElfSymbolDefinition::Undefined,
            },
            value: match &symbol.value {
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
    conversion.insert(symbol.id, id);
}
