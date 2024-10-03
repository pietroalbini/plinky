use crate::repr::object::Object;
use crate::repr::symbols::views::AllSymbols;
use crate::repr::symbols::SymbolVisibility;

/// A STV_HIDDEN or STV_INTERNAL symbol will be made STB_LOCAL in the linker output
///
/// https://maskray.me/blog/2021-06-20-symbol-processing
pub(crate) fn run(object: &mut Object) {
    for symbol in object.symbols.iter_mut(&AllSymbols) {
        if let SymbolVisibility::Global { hidden: true, .. } = symbol.visibility() {
            symbol.set_visibility(SymbolVisibility::Local);
        }
    }
}
