use crate::cli::Mode;
use crate::repr::object::Object;
use crate::repr::symbols::views::AllSymbols;
use crate::repr::symbols::SymbolVisibility;

pub(crate) fn run(object: &mut Object) {
    match object.mode {
        Mode::PositionDependent | Mode::PositionIndependent => return,
        Mode::SharedLibrary => {}
    }

    for (_, symbol) in object.symbols.iter_mut(&AllSymbols) {
        if let SymbolVisibility::Global { hidden: false, .. } = symbol.visibility() {
            symbol.mark_needed_by_dynamic();
        }
    }
}
