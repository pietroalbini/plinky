use crate::repr::symbols::Symbol;
use plinky_macros::Display;
use std::fmt::{Debug, Display};

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
#[display("symbol table")]
pub(crate) struct SymbolTable;

impl SymbolsView for SymbolTable {
    fn filter(&self, symbol: &Symbol) -> bool {
        !symbol.exclude_from_tables()
    }
}

#[derive(Clone, Copy, Debug, Display)]
#[display("dynamic symbol table")]
pub(crate) struct DynamicSymbolTable;

impl SymbolsView for DynamicSymbolTable {
    fn filter(&self, symbol: &Symbol) -> bool {
        symbol.needed_by_dynamic() && !symbol.exclude_from_tables()
    }
}
