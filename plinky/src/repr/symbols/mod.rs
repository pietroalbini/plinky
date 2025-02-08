mod symbol;
pub(crate) mod views;

use crate::interner::Interned;
use plinky_elf::ElfSymbolVisibility;
use plinky_macros::{Display, Error};
use std::collections::{BTreeMap, btree_map};

use crate::repr::sections::SectionId;
use crate::repr::symbols::views::SymbolsView;
pub(crate) use symbol::{
    ResolveSymbolError, ResolvedSymbol, Symbol, SymbolType, SymbolValue, SymbolVisibility,
    UpcomingSymbol,
};

const NULL_SYMBOL_ID: SymbolId = SymbolId(0);

#[derive(Debug)]
pub(crate) struct Symbols {
    symbols: Vec<SymbolSlot>,
    section_symbols: BTreeMap<SectionId, SymbolId>,
    global_symbols: BTreeMap<Interned<String>, SymbolId>,
}

impl Symbols {
    pub(crate) fn new() -> Result<Self, LoadSymbolsError> {
        let mut symbols = Self {
            symbols: Vec::new(),
            section_symbols: BTreeMap::new(),
            global_symbols: BTreeMap::new(),
        };

        let null_symbol_id = symbols.add(UpcomingSymbol::Null)?;
        assert_eq!(NULL_SYMBOL_ID, null_symbol_id);

        Ok(symbols)
    }

    pub(crate) fn add(&mut self, upcoming: UpcomingSymbol) -> Result<SymbolId, LoadSymbolsError> {
        match upcoming.visibility()? {
            SymbolVisibility::Local => {
                let id = SymbolId(self.symbols.len());
                let symbol = upcoming.create(id)?;

                if let SymbolValue::Section { section } = &symbol.value() {
                    self.section_symbols.insert(*section, id);
                }

                self.symbols.push(SymbolSlot::Present(symbol));
                Ok(id)
            }
            SymbolVisibility::Global { weak: false, hidden: _ } => {
                match self.global_symbols.entry(upcoming.name()) {
                    btree_map::Entry::Vacant(entry) => {
                        let id = SymbolId(self.symbols.len());
                        self.symbols.push(SymbolSlot::Present(upcoming.create(id)?));
                        entry.insert(id);
                        Ok(id)
                    }
                    btree_map::Entry::Occupied(entry) => match &mut self.symbols[entry.get().0] {
                        SymbolSlot::Removed => panic!("global symbol points to a removed symbol"),
                        SymbolSlot::Present(existing) => {
                            if let SymbolValue::Undefined = existing.value() {
                                *existing = upcoming.create(existing.id())?;
                                Ok(existing.id())
                            } else if let SymbolValue::Undefined = upcoming.value() {
                                Ok(existing.id())
                            } else {
                                return Err(LoadSymbolsError::DuplicateGlobalSymbol(
                                    upcoming.name(),
                                ));
                            }
                        }
                    },
                }
            }
            SymbolVisibility::Global { weak: true, hidden: _ } => {
                todo!("weak symbols are not supported yet")
            }
        }
    }

    pub(crate) fn remove(&mut self, id: SymbolId) {
        let old = std::mem::replace(&mut self.symbols[id.0], SymbolSlot::Removed);

        if let SymbolSlot::Present(old) = old {
            if let SymbolValue::Section { section } = old.value() {
                self.section_symbols.remove(&section);
            }
        }
    }

    pub(crate) fn get(&self, id: SymbolId) -> &Symbol {
        match self.symbols.get(id.0) {
            Some(SymbolSlot::Present(symbol)) => symbol,
            Some(SymbolSlot::Removed) => panic!("trying to access a placeholder symbol"),
            None => panic!("trying to access a missing symbol"),
        }
    }

    pub(crate) fn get_mut(&mut self, id: SymbolId) -> &mut Symbol {
        match self.symbols.get_mut(id.0) {
            Some(SymbolSlot::Present(symbol)) => symbol,
            Some(SymbolSlot::Removed) => panic!("trying to access a placeholder symbol"),
            None => panic!("trying to access a missing symbol"),
        }
    }

    pub(crate) fn get_global(
        &self,
        name: Interned<String>,
    ) -> Result<&Symbol, MissingGlobalSymbol> {
        Ok(self.get(*self.global_symbols.get(&name).ok_or(MissingGlobalSymbol { name })?))
    }

    pub(crate) fn section_symbol_id(&self, id: SectionId) -> Option<SymbolId> {
        self.section_symbols.get(&id).copied()
    }

    pub(crate) fn iter<'a>(
        &'a self,
        view: &'a dyn SymbolsView,
    ) -> Box<dyn Iterator<Item = &'a Symbol> + 'a> {
        let iter = self
            .symbols
            .iter()
            .filter_map(|symbol| match symbol {
                SymbolSlot::Present(symbol) => Some(symbol),
                SymbolSlot::Removed => None,
            })
            .filter(|symbol| symbol.is_null_symbol() || view.filter(symbol));

        if view.should_sort() {
            let mut symbols = iter.collect::<Vec<_>>();
            view.sort_ref(&mut symbols);
            Box::new(symbols.into_iter())
        } else {
            Box::new(iter)
        }
    }

    pub(crate) fn iter_mut<'a>(
        &'a mut self,
        view: &'a dyn SymbolsView,
    ) -> Box<dyn Iterator<Item = &'a mut Symbol> + 'a> {
        let iter = self
            .symbols
            .iter_mut()
            .filter_map(|symbol| match symbol {
                SymbolSlot::Present(symbol) => Some(symbol),
                SymbolSlot::Removed => None,
            })
            .filter(|symbol| symbol.is_null_symbol() || view.filter(symbol));

        if view.should_sort() {
            let mut symbols = iter.collect::<Vec<_>>();
            view.sort_mut(&mut symbols);
            Box::new(symbols.into_iter())
        } else {
            Box::new(iter)
        }
    }

    pub(crate) fn null_symbol_id(&self) -> SymbolId {
        NULL_SYMBOL_ID
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct SymbolId(usize);

#[derive(Debug)]
enum SymbolSlot {
    Removed,
    Present(Symbol),
}

#[derive(Debug, Error, Display)]
#[display("missing global symbol: {name}")]
pub(crate) struct MissingGlobalSymbol {
    name: Interned<String>,
}

#[derive(Debug, Error, Display, PartialEq, Eq)]
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
    #[display("duplicate global symbol {f0}")]
    DuplicateGlobalSymbol(Interned<String>),
}
