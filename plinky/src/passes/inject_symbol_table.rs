use crate::repr::object::Object;
use crate::repr::sections::{StringsSection, SymbolsSection};
use crate::repr::symbols::views::SymbolTable;
use plinky_elf::ids::serial::SerialIds;

pub(crate) fn run(object: &mut Object, ids: &mut SerialIds) {
    let string_table_id = ids.allocate_section_id();
    object
        .sections
        .builder(".strtab", StringsSection::new(SymbolTable))
        .create_with_id(string_table_id);
    object
        .sections
        .builder(".symtab", SymbolsSection::new(string_table_id, SymbolTable, false))
        .create(ids);
}
