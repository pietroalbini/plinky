use crate::repr::object::Object;
use crate::repr::sections::{StringsSection, SymbolsSection};
use crate::repr::symbols::views::SymbolTable;

pub(crate) fn run(object: &mut Object) {
    let string_table_id =
        object.sections.builder(".strtab", StringsSection::new(SymbolTable)).create();
    object
        .sections
        .builder(".symtab", SymbolsSection::new(string_table_id, SymbolTable, false))
        .create();
}
