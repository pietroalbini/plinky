use crate::repr::symbols::Symbol;
use std::fmt::{Debug, Display};
use plinky_macros::Display;

pub(crate) trait SymbolsView: Debug + Display {
    fn filter(&self, symbol: &Symbol) -> bool;
}

#[derive(Clone, Copy, Debug, Display)]
#[display("all symbols")]
pub(crate) struct AllSymbols;

impl SymbolsView for AllSymbols {
    fn filter(&self, _: &Symbol) -> bool {
        true
    }
}

#[derive(Clone, Copy, Debug, Display)]
#[display("dynamic symbols")]
pub(crate) struct DynamicSymbols;

impl SymbolsView for DynamicSymbols {
    fn filter(&self, symbol: &Symbol) -> bool {
        symbol.needed_by_dynamic
    }
}
