use crate::passes::build_elf::gnu_hash::{GnuHashResult, GnuHasher};
use crate::repr::symbols::Symbol;
use plinky_elf::ElfClass;
use plinky_macros::Display;
use std::fmt::{Debug, Display};

pub(crate) trait SymbolsView: Debug + Display {
    fn filter(&self, symbol: &Symbol) -> bool;

    fn should_sort(&self) -> bool {
        false
    }

    fn sort_ref(&self, _symbols: &mut Vec<&Symbol>) {
        unimplemented!();
    }

    fn sort_mut(&self, _symbols: &mut Vec<&mut Symbol>) {
        unimplemented!();
    }
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
pub(crate) struct DynamicSymbolTable {
    pub(crate) class: ElfClass,
}

impl SymbolsView for DynamicSymbolTable {
    fn filter(&self, symbol: &Symbol) -> bool {
        symbol.needed_by_dynamic() && !symbol.exclude_from_tables()
    }

    fn should_sort(&self) -> bool {
        true
    }

    fn sort_ref(&self, symbols: &mut Vec<&Symbol>) {
        let hasher = GnuHasher::new(self.class, symbols.iter().map(|s| *s));
        symbols.sort_by_cached_key(|s| gnu_hash_sorting_key(*s, &hasher));
    }

    fn sort_mut(&self, symbols: &mut Vec<&mut Symbol>) {
        let hasher = GnuHasher::new(self.class, symbols.iter().map(|s| &**s));
        symbols.sort_by_cached_key(|s| gnu_hash_sorting_key(*s, &hasher));
    }
}

fn gnu_hash_sorting_key(symbol: &Symbol, hasher: &GnuHasher) -> (u32, u32) {
    match hasher.hash(symbol) {
        // Place items that should not be hashed before items that will be hashed, and group the
        // hashed items together by bucket. This is needed by the GNU Hash format.
        GnuHashResult::NotHashed => (0, 0),
        GnuHashResult::Hashed { bucket, .. } => (1, bucket),
    }
}
