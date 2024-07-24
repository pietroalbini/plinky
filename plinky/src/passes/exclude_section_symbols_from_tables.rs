use crate::repr::object::Object;
use crate::repr::symbols::views::AllSymbols;
use crate::repr::symbols::SymbolType;

pub(crate) fn remove(object: &mut Object) {
    for (_id, symbol) in object.symbols.iter_mut(&AllSymbols) {
        match &symbol.type_ {
            SymbolType::Section => symbol.exclude_from_tables = true,
            _ => {}
        }
    }
}
