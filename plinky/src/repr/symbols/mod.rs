mod symbol;
pub(crate) mod views;

use crate::interner::Interned;
use plinky_elf::ids::serial::{SerialIds, SymbolId};
use plinky_elf::ElfSymbolVisibility;
use plinky_macros::{Display, Error};
use std::collections::{btree_map, BTreeMap};

use crate::repr::symbols::views::SymbolsView;
pub(crate) use symbol::{
    ResolveSymbolError, ResolvedSymbol, Symbol, SymbolType, SymbolValue, SymbolVisibility,
};

#[derive(Debug)]
pub(crate) struct Symbols {
    null_symbol_id: SymbolId,
    symbols: BTreeMap<SymbolId, SymbolOrRedirect>,
    global_symbols: BTreeMap<Interned<String>, SymbolId>,
    frozen: bool,
}

impl Symbols {
    pub(crate) fn new(ids: &mut SerialIds) -> Self {
        let null_symbol_id = ids.allocate_symbol_id();

        let mut symbols = BTreeMap::new();
        symbols.insert(null_symbol_id, SymbolOrRedirect::Symbol(Symbol::new_null(null_symbol_id)));
        Self { null_symbol_id, symbols, global_symbols: BTreeMap::new(), frozen: false }
    }

    pub(crate) fn freeze(&mut self) {
        self.frozen = true;
    }

    pub(crate) fn add_unknown_global(
        &mut self,
        ids: &mut SerialIds,
        name: &str,
    ) -> Result<SymbolId, LoadSymbolsError> {
        let id = ids.allocate_symbol_id();
        self.add_symbol(Symbol::new_global_unknown(id, name))?;
        Ok(id)
    }

    pub(crate) fn add_redirect(&mut self, from: SymbolId, to: SymbolId) {
        if self.frozen {
            panic!("trying to add a redirect with frozen symbols");
        }
        self.symbols.insert(from, SymbolOrRedirect::Redirect(to));
    }

    pub(crate) fn add_symbol(&mut self, mut symbol: Symbol) -> Result<(), LoadSymbolsError> {
        if self.frozen {
            panic!("trying to add a symbol with frozen symbols");
        }
        match symbol.visibility() {
            SymbolVisibility::Local => {
                self.symbols.insert(symbol.id(), SymbolOrRedirect::Symbol(symbol));
            }
            SymbolVisibility::Global { weak: false, hidden: _ } => {
                // For global symbols, we generate a new symbol ID for each unique name, and
                // redirect to it all of the concrete references to that global name.
                let global_id = *self.global_symbols.entry(symbol.name()).or_insert(symbol.id());
                if symbol.id() != global_id {
                    self.add_redirect(symbol.id(), global_id);
                }

                // Ensure the ID contained in the symbol is the global ID, not the original ID.
                symbol.set_id(global_id);

                match self.symbols.entry(global_id) {
                    btree_map::Entry::Vacant(entry) => {
                        entry.insert(SymbolOrRedirect::Symbol(symbol));
                    }
                    btree_map::Entry::Occupied(mut entry) => {
                        let SymbolOrRedirect::Symbol(existing_symbol) = entry.get() else {
                            panic!("global symbols can't be a redirect");
                        };
                        if let SymbolValue::Undefined = existing_symbol.value() {
                            entry.insert(SymbolOrRedirect::Symbol(symbol));
                        } else if let SymbolValue::Undefined = symbol.value() {
                            // Nothing.
                        } else {
                            return Err(LoadSymbolsError::DuplicateGlobalSymbol(symbol.name()));
                        }
                    }
                }
            }
            SymbolVisibility::Global { weak: true, hidden: _ } => {
                todo!("weak symbols are not supported yet")
            }
        }
        Ok(())
    }

    pub(crate) fn remove(&mut self, id: SymbolId) {
        if self.frozen {
            panic!("trying to remove a symbol with frozen symbols");
        }
        self.symbols.remove(&id);
    }

    pub(crate) fn get(&self, mut id: SymbolId) -> &Symbol {
        let mut attempts = 0;
        while attempts < 10 {
            match self.symbols.get(&id) {
                Some(SymbolOrRedirect::Symbol(symbol)) => return symbol,
                Some(SymbolOrRedirect::Redirect(redirect)) => id = *redirect,
                None => panic!("symbol id doesn't point to a symbol"),
            }
            attempts += 1;
        }
        panic!("too many redirects while resolving symbol {id:?}");
    }

    pub(crate) fn get_mut(&mut self, id: SymbolId) -> &mut Symbol {
        let id = self.get(id).id(); // Resolve redirects.
        match self.symbols.get_mut(&id).unwrap() {
            SymbolOrRedirect::Symbol(symbol) => symbol,
            SymbolOrRedirect::Redirect(_) => unreachable!(),
        }
    }

    pub(crate) fn get_global(
        &self,
        name: Interned<String>,
    ) -> Result<&Symbol, MissingGlobalSymbol> {
        Ok(self.get(*self.global_symbols.get(&name).ok_or(MissingGlobalSymbol { name })?))
    }

    pub(crate) fn iter<'a>(
        &'a self,
        view: &'a dyn SymbolsView,
    ) -> impl Iterator<Item = (SymbolId, &'a Symbol)> + 'a {
        self.symbols
            .iter()
            .filter_map(|(id, symbol)| match symbol {
                SymbolOrRedirect::Symbol(symbol) => Some((*id, symbol)),
                SymbolOrRedirect::Redirect(_) => None,
            })
            .filter(move |symbol| symbol.0 == self.null_symbol_id || view.filter(symbol.1))
    }

    pub(crate) fn iter_mut<'a>(
        &'a mut self,
        view: &'a dyn SymbolsView,
    ) -> impl Iterator<Item = (SymbolId, &'a mut Symbol)> + 'a {
        let null_symbol_id = self.null_symbol_id;
        self.symbols
            .iter_mut()
            .filter_map(|(id, symbol)| match symbol {
                SymbolOrRedirect::Symbol(symbol) => Some((*id, symbol)),
                SymbolOrRedirect::Redirect(_) => None,
            })
            .filter(move |symbol| symbol.0 == null_symbol_id || view.filter(symbol.1))
    }

    pub(crate) fn iter_with_redirects<'a>(
        &'a self,
        view: &'a dyn SymbolsView,
    ) -> impl Iterator<Item = (SymbolId, &'a Symbol)> + 'a {
        self.symbols
            .keys()
            .map(|&id| (id, self.get(id)))
            .filter(move |symbol| symbol.0 == self.null_symbol_id || view.filter(symbol.1))
    }

    pub(crate) fn null_symbol_id(&self) -> SymbolId {
        self.null_symbol_id
    }
}

#[derive(Debug)]
enum SymbolOrRedirect {
    Symbol(Symbol),
    Redirect(SymbolId),
}

#[derive(Debug, Error, Display)]
#[display("missing global symbol: {name}")]
pub(crate) struct MissingGlobalSymbol {
    name: Interned<String>,
}

#[derive(Debug, Error, Display)]
pub(crate) enum LoadSymbolsError {
    #[display("unknown symbol bindings are not supported")]
    UnsupportedUnknownSymbolBinding,
    #[display("unknown symbol types are not supported")]
    UnsupportedUnknownSymbolType,
    #[display("file symbol types are not supported")]
    UnsupportedFileSymbolType,
    #[display("unsupported symbol visibility {f0:?}")]
    UnsupportedVisibility(ElfSymbolVisibility),
    #[display("local symbols cannot have hidden visibility")]
    LocalHiddenSymbol,
    #[display("missing name for symbol {f0:?}")]
    MissingSymbolName(SymbolId),
    #[display("duplicate global symbol {f0}")]
    DuplicateGlobalSymbol(Interned<String>),
}
