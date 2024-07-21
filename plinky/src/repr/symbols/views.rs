use crate::repr::symbols::Symbol;

pub(crate) trait SymbolsView {
    fn is_dynamic(&self) -> bool;
    fn filter(&self, symbol: &Symbol) -> bool;
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct AllSymbols;

impl SymbolsView for AllSymbols {
    fn is_dynamic(&self) -> bool {
        false
    }

    fn filter(&self, _: &Symbol) -> bool {
        true
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct DynamicSymbols;

impl SymbolsView for DynamicSymbols {
    fn is_dynamic(&self) -> bool {
        true
    }

    fn filter(&self, symbol: &Symbol) -> bool {
        symbol.needed_by_dynamic
    }
}
