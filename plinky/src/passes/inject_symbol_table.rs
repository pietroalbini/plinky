use crate::repr::object::Object;
use crate::repr::sections::{StringsForSymbolsSection, SymbolsSection};
use crate::repr::symbols::views::AllSymbols;
use plinky_elf::ids::serial::SerialIds;

pub(crate) fn run(object: &mut Object, ids: &mut SerialIds) {
    let string_table_id = ids.allocate_section_id();
    object
        .sections
        .builder(".strtab", StringsForSymbolsSection::new(AllSymbols))
        .create_with_id(string_table_id);
    object
        .sections
        .builder(".symtab", SymbolsSection::new(string_table_id, AllSymbols, false))
        .create(ids);
}
