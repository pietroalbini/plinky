use crate::interner::{intern, Interned};
use crate::passes::layout::{AddressResolutionError, Layout};
use crate::repr::strings::{MissingStringError, Strings};
use plinky_diagnostics::ObjectSpan;
use plinky_elf::ids::serial::{SectionId, SerialIds, SymbolId};
use plinky_elf::{ElfSymbolBinding, ElfSymbolDefinition, ElfSymbolTable, ElfSymbolType};
use plinky_macros::{Display, Error};
use std::collections::{btree_map, BTreeMap};

#[derive(Debug)]
pub(crate) struct Symbols {
    symbols: BTreeMap<SymbolId, SymbolOrRedirect>,
    global_symbols: BTreeMap<Interned<String>, SymbolId>,
}

impl Symbols {
    pub(crate) fn new() -> Self {
        Self { symbols: BTreeMap::new(), global_symbols: BTreeMap::new() }
    }

    pub(crate) fn add_unknown_global(
        &mut self,
        ids: &mut SerialIds,
        name: &str,
    ) -> Result<SymbolId, LoadSymbolsError> {
        let id = ids.allocate_symbol_id();
        self.add_symbol(
            ids,
            id,
            Symbol {
                id,
                name: intern(name),
                type_: SymbolType::NoType,
                stt_file: None,
                span: intern(ObjectSpan::new_synthetic()),
                visibility: SymbolVisibility::Global { weak: false },
                value: SymbolValue::Undefined,
            },
        )?;
        Ok(id)
    }

    pub(crate) fn load_table(
        &mut self,
        ids: &mut SerialIds,
        span: Interned<ObjectSpan>,
        table: ElfSymbolTable<SerialIds>,
        strings: &Strings,
    ) -> Result<(), LoadSymbolsError> {
        let mut stt_file = None;
        for (symbol_id, elf_symbol) in table.symbols.into_iter() {
            let name = intern(
                strings
                    .get(elf_symbol.name)
                    .map_err(|e| LoadSymbolsError::MissingSymbolName(symbol_id, e))?,
            );

            let type_ = match elf_symbol.type_ {
                ElfSymbolType::NoType => SymbolType::NoType,
                ElfSymbolType::Object => SymbolType::Object,
                ElfSymbolType::Function => SymbolType::Function,
                ElfSymbolType::Section => SymbolType::Section,
                // The file symbol type is not actually used, so we can omit it.
                ElfSymbolType::File => {
                    file = Some(name);
                    continue;
                },
                ElfSymbolType::Unknown(_) => {
                    return Err(LoadSymbolsError::UnsupportedUnknownSymbolType)
                }
            };

            let symbol = Symbol {
                id: symbol_id,
                name,
                type_,
                stt_file: file,
                span,
                visibility: match elf_symbol.binding {
                    ElfSymbolBinding::Local => SymbolVisibility::Local,
                    ElfSymbolBinding::Global => SymbolVisibility::Global { weak: false },
                    ElfSymbolBinding::Weak => SymbolVisibility::Global { weak: true },
                    ElfSymbolBinding::Unknown(_) => {
                        return Err(LoadSymbolsError::UnsupportedUnknownSymbolBinding);
                    }
                },
                value: match elf_symbol.definition {
                    ElfSymbolDefinition::Undefined => SymbolValue::Undefined,
                    ElfSymbolDefinition::Absolute => {
                        SymbolValue::Absolute { value: elf_symbol.value }
                    }
                    ElfSymbolDefinition::Common => todo!(),
                    ElfSymbolDefinition::Section(section) => {
                        SymbolValue::SectionRelative { section, offset: elf_symbol.value }
                    }
                },
            };

            self.add_symbol(ids, symbol_id, symbol)?;
        }
        Ok(())
    }

    fn add_symbol(
        &mut self,
        ids: &mut SerialIds,
        id: SymbolId,
        mut symbol: Symbol,
    ) -> Result<(), LoadSymbolsError> {
        match symbol.visibility {
            SymbolVisibility::Local => {
                self.symbols.insert(id, SymbolOrRedirect::Symbol(symbol));
            }
            SymbolVisibility::Global { weak: false } => {
                // For global symbols, we generate a new symbol ID for each unique name, and
                // redirect to it all of the concrete references to that global name.
                let global_id = *self
                    .global_symbols
                    .entry(symbol.name)
                    .or_insert_with(|| ids.allocate_symbol_id());
                self.symbols.insert(id, SymbolOrRedirect::Redirect(global_id));

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
            SymbolVisibility::Global { weak: true } => {
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

    pub(crate) fn iter(&self) -> impl Iterator<Item = (SymbolId, &Symbol)> {
        self.symbols.iter().filter_map(|(id, symbol)| match symbol {
            SymbolOrRedirect::Symbol(symbol) => Some((*id, symbol)),
            SymbolOrRedirect::Redirect(_) => None,
        })
    }

    pub(crate) fn iters_with_redirects(&self) -> impl Iterator<Item = (SymbolId, &Symbol)> {
        self.symbols.keys().map(|&id| (id, self.get(id)))
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
        offset: i64,
    ) -> Result<ResolvedSymbol, ResolveSymbolError> {
        match &self.value {
            SymbolValue::Undefined => Err(ResolveSymbolError {
                symbol: self.name,
                inner: ResolveSymbolErrorKind::Undefined,
            }),
            SymbolValue::Absolute { value } => Ok(ResolvedSymbol::Absolute(*value)),
            SymbolValue::SectionRelative { section, offset: section_offset } => {
                match layout.address(*section, (*section_offset as i64) + offset) {
                    Ok(result) => Ok(ResolvedSymbol::Address(result)),
                    Err(err) => Err(ResolveSymbolError {
                        symbol: self.name,
                        inner: ResolveSymbolErrorKind::Layout(err),
                    }),
                }
            }
        }
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
    Global { weak: bool },
}

#[derive(Debug)]
pub(crate) enum SymbolValue {
    Absolute { value: u64 },
    SectionRelative { section: SectionId, offset: u64 },
    Undefined,
}

pub(crate) enum ResolvedSymbol {
    Absolute(u64),
    Address(u64),
}

impl ResolvedSymbol {
    pub(crate) fn as_u64(&self) -> u64 {
        match self {
            ResolvedSymbol::Absolute(v) => *v,
            ResolvedSymbol::Address(v) => *v,
        }
    }
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
    #[display("missing name for symbol {f0:?}")]
    MissingSymbolName(SymbolId, #[source] MissingStringError),
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
    #[display("symbol is not defined")]
    Undefined,
    #[transparent]
    Layout(AddressResolutionError),
}
