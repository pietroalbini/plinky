use crate::interner::Interned;
use crate::passes::build_elf::ids::{BuiltElfIds, BuiltElfStringId};
use crate::passes::build_elf::{ElfBuilder, StringsTableBuilder};
use crate::repr::sections::{StringsSection, UpcomingStringId};
use crate::repr::symbols::{SymbolId, SymbolVisibility};
use plinky_elf::ids::serial::SectionId;
use plinky_elf::ElfSectionContent;
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn create_strings(
    builder: &ElfBuilder<'_>,
    id: SectionId,
    strings_section: &StringsSection,
) -> BuiltStringsTable {
    let mut table = StringsTableBuilder::new(*builder.section_ids.get(&id).unwrap());

    let mut custom_strings = BTreeMap::new();
    for (custom_string_id, custom_string) in strings_section.iter_custom_strings() {
        custom_strings.insert(custom_string_id, table.add(custom_string));
    }

    let mut symbol_names = BTreeMap::new();
    let mut found_file_names = BTreeSet::new();
    for symbol in builder.object.symbols.iter(strings_section.symbol_names_view()) {
        symbol_names.insert(symbol.id(), table.add(symbol.name().resolve().as_str()));

        if let (Some(name), SymbolVisibility::Local) = (symbol.stt_file(), symbol.visibility()) {
            // File names are added to an intermediate set to deduplicate them.
            found_file_names.insert(name);
        }
    }

    let mut symbol_file_names = BTreeMap::new();
    for file_name in found_file_names {
        symbol_file_names.insert(file_name, table.add(file_name.resolve().as_str()));
    }

    BuiltStringsTable {
        elf: Some(table.into_elf()),
        symbol_names,
        symbol_file_names,
        custom_strings,
    }
}

pub(super) struct BuiltStringsTable {
    pub(super) elf: Option<ElfSectionContent<BuiltElfIds>>,
    pub(super) symbol_file_names: BTreeMap<Interned<String>, BuiltElfStringId>,
    pub(super) symbol_names: BTreeMap<SymbolId, BuiltElfStringId>,
    pub(super) custom_strings: BTreeMap<UpcomingStringId, BuiltElfStringId>,
}
