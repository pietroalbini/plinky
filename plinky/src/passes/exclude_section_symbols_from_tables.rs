use crate::repr::object::Object;
use crate::repr::symbols::views::AllSymbols;
use crate::repr::symbols::SymbolValue;

pub(crate) fn remove(object: &mut Object) {
    for symbol in object.symbols.iter_mut(&AllSymbols) {
        match &symbol.value() {
            SymbolValue::Section { .. } => symbol.mark_exclude_from_tables(),
            _ => {}
        }
    }
}
