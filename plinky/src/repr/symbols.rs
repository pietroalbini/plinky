use crate::interner::{intern, Interned};
use crate::passes::layout::{AddressResolutionError, Layout};
use crate::utils::ints::{Absolute, Address, Offset, OutOfBoundsError};
use plinky_diagnostics::ObjectSpan;
use plinky_elf::ids::serial::{SectionId, SerialIds, SymbolId};
use plinky_elf::ElfSymbolVisibility;
use plinky_macros::{Display, Error};
use std::collections::{btree_map, BTreeMap, BTreeSet};

#[derive(Debug)]
pub(crate) struct Symbols {
    null_symbol_id: SymbolId,
    symbols: BTreeMap<SymbolId, SymbolOrRedirect>,
    global_symbols: BTreeMap<Interned<String>, SymbolId>,
    dynamic_symbols: BTreeSet<SymbolId>,
}

impl Symbols {
    pub(crate) fn new(ids: &mut SerialIds) -> Self {
        let null_symbol_id = ids.allocate_symbol_id();

        let mut symbols = BTreeMap::new();
        symbols.insert(
            null_symbol_id,
            SymbolOrRedirect::Symbol(Symbol {
                id: null_symbol_id,
                name: intern(""),
                type_: SymbolType::NoType,
                stt_file: None,
                span: intern(ObjectSpan::new_synthetic()),
                visibility: SymbolVisibility::Local,
                value: SymbolValue::Null,
            }),
        );
        Self {
            null_symbol_id,
            symbols,
            global_symbols: BTreeMap::new(),
            dynamic_symbols: BTreeSet::new(),
        }
    }

    pub(crate) fn add_unknown_global(
        &mut self,
        ids: &mut SerialIds,
        name: &str,
    ) -> Result<SymbolId, LoadSymbolsError> {
        let id = ids.allocate_symbol_id();
        self.add_symbol(Symbol {
            id,
            name: intern(name),
            type_: SymbolType::NoType,
            stt_file: None,
            span: intern(ObjectSpan::new_synthetic()),
            visibility: SymbolVisibility::Global { weak: false, hidden: false },
            value: SymbolValue::Undefined,
        })?;
        Ok(id)
    }

    pub(crate) fn add_redirect(&mut self, from: SymbolId, to: SymbolId) {
        self.symbols.insert(from, SymbolOrRedirect::Redirect(to));
    }

    pub(crate) fn add_symbol(&mut self, mut symbol: Symbol) -> Result<(), LoadSymbolsError> {
        match symbol.visibility {
            SymbolVisibility::Local => {
                self.symbols.insert(symbol.id, SymbolOrRedirect::Symbol(symbol));
            }
            SymbolVisibility::Global { weak: false, hidden: _ } => {
                // For global symbols, we generate a new symbol ID for each unique name, and
                // redirect to it all of the concrete references to that global name.
                let global_id = *self.global_symbols.entry(symbol.name).or_insert(symbol.id);
                if symbol.id != global_id {
                    self.add_redirect(symbol.id, global_id);
                }

                // Ensure the ID contained in the symbol is the global ID, not the original ID.
                symbol.id = global_id;

                match self.symbols.entry(global_id) {
                    btree_map::Entry::Vacant(entry) => {
                        entry.insert(SymbolOrRedirect::Symbol(symbol));
                    }
                    btree_map::Entry::Occupied(mut entry) => {
                        let SymbolOrRedirect::Symbol(existing_symbol) = entry.get() else {
                            panic!("global symbols can't be a redirect");
                        };
                        if let SymbolValue::Undefined = existing_symbol.value {
                            entry.insert(SymbolOrRedirect::Symbol(symbol));
                        } else if let SymbolValue::Undefined = symbol.value {
                            // Nothing.
                        } else {
                            return Err(LoadSymbolsError::DuplicateGlobalSymbol(symbol.name));
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

    pub(crate) fn get_global(
        &self,
        name: Interned<String>,
    ) -> Result<&Symbol, MissingGlobalSymbol> {
        Ok(self.get(*self.global_symbols.get(&name).ok_or(MissingGlobalSymbol { name })?))
    }

    pub(crate) fn add_symbol_to_dynamic(&mut self, id: SymbolId) {
        self.dynamic_symbols.insert(id);
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (SymbolId, &Symbol)> {
        self.symbols.iter().filter_map(|(id, symbol)| match symbol {
            SymbolOrRedirect::Symbol(symbol) => Some((*id, symbol)),
            SymbolOrRedirect::Redirect(_) => None,
        })
    }

    pub(crate) fn iter_mut(&mut self) -> impl Iterator<Item = (SymbolId, &mut Symbol)> {
        self.symbols.iter_mut().filter_map(|(id, symbol)| match symbol {
            SymbolOrRedirect::Symbol(symbol) => Some((*id, symbol)),
            SymbolOrRedirect::Redirect(_) => None,
        })
    }

    pub(crate) fn iters_with_redirects(&self) -> impl Iterator<Item = (SymbolId, &Symbol)> {
        self.symbols.keys().map(|&id| (id, self.get(id)))
    }

    pub(crate) fn iter_dynamic_symbols(&self) -> impl Iterator<Item = (SymbolId, &Symbol)> {
        self.iter().filter(|(_, symbol)| {
            self.dynamic_symbols.contains(&symbol.id) || symbol.id == self.null_symbol_id
        })
    }

    pub(crate) fn has_dynamic_symbols(&self) -> bool {
        self.dynamic_symbols.len() > 0
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

#[derive(Debug)]
pub(crate) struct Symbol {
    pub(crate) id: SymbolId,
    pub(crate) name: Interned<String>,
    pub(crate) type_: SymbolType,
    pub(crate) stt_file: Option<Interned<String>>,
    pub(crate) span: Interned<ObjectSpan>,
    pub(crate) visibility: SymbolVisibility,
    pub(crate) value: SymbolValue,
}

impl Symbol {
    pub(crate) fn resolve(
        &self,
        layout: &Layout,
        offset: Offset,
    ) -> Result<ResolvedSymbol, ResolveSymbolError> {
        fn resolve_inner(
            symbol: &Symbol,
            layout: &Layout,
            offset: Offset,
        ) -> Result<ResolvedSymbol, ResolveSymbolErrorKind> {
            match &symbol.value {
                SymbolValue::Undefined => Err(ResolveSymbolErrorKind::Undefined),
                SymbolValue::Absolute { value } => {
                    assert!(offset == Offset::from(0));
                    Ok(ResolvedSymbol::Absolute(*value))
                }
                SymbolValue::SectionRelative { section, offset: section_offset } => {
                    match layout.address(*section, section_offset.add(offset)?) {
                        Ok((section, memory_address)) => {
                            Ok(ResolvedSymbol::Address { section, memory_address })
                        }
                        Err(err) => Err(ResolveSymbolErrorKind::Layout(err)),
                    }
                }
                SymbolValue::SectionVirtualAddress { section, memory_address } => {
                    Ok(ResolvedSymbol::Address {
                        section: *section,
                        memory_address: memory_address.offset(offset)?,
                    })
                }
                SymbolValue::Null => Err(ResolveSymbolErrorKind::Null),
            }
        }

        resolve_inner(self, layout, offset)
            .map_err(|inner| ResolveSymbolError { symbol: self.name, inner })
    }
}

#[derive(Debug)]
pub(crate) enum SymbolType {
    NoType,
    Function,
    Object,
    Section,
}

#[derive(Debug)]
pub(crate) enum SymbolVisibility {
    Local,
    Global { weak: bool, hidden: bool },
}

#[derive(Debug)]
pub(crate) enum SymbolValue {
    Absolute { value: Absolute },
    SectionRelative { section: SectionId, offset: Offset },
    SectionVirtualAddress { section: SectionId, memory_address: Address },
    Undefined,
    Null,
}

pub(crate) enum ResolvedSymbol {
    Absolute(Absolute),
    Address { section: SectionId, memory_address: Address },
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
    #[display("unsupported symbol visibility {f0:?}")]
    UnsupportedVisibility(ElfSymbolVisibility),
    #[display("local symbols cannot have hidden visibility")]
    LocalHiddenSymbol,
    #[display("missing name for symbol {f0:?}")]
    MissingSymbolName(SymbolId),
    #[display("duplicate global symbol {f0}")]
    DuplicateGlobalSymbol(Interned<String>),
}

#[derive(Debug, Error, Display)]
#[display("failed to resolve symbol {symbol}")]
pub(crate) struct ResolveSymbolError {
    pub(crate) symbol: Interned<String>,
    #[source]
    pub(crate) inner: ResolveSymbolErrorKind,
}

#[derive(Debug, Error, Display)]
pub(crate) enum ResolveSymbolErrorKind {
    #[display("the symbol is the null symbol")]
    Null,
    #[display("symbol is not defined")]
    Undefined,
    #[transparent]
    Layout(AddressResolutionError),
    #[transparent]
    OutOfBounds(OutOfBoundsError),
}
