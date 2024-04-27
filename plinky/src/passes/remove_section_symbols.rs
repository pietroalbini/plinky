use crate::repr::object::Object;
use crate::repr::symbols::SymbolType;

pub(crate) fn remove(object: &mut Object) {
    let mut to_remove = Vec::new();
    for (id, symbol) in object.symbols.iter() {
        match &symbol.type_ {
            SymbolType::Section => to_remove.push(id),
            _ => {}
        }
    }
    for id in to_remove {
        object.symbols.remove(id);
    }
}
