use crate::interner::{intern, Interned};
use crate::repr::strings::{MissingStringError, Strings};
use plinky_diagnostics::ObjectSpan;
use plinky_elf::ids::serial::{SectionId, SerialIds, SymbolId};
use plinky_elf::{ElfSymbolBinding, ElfSymbolDefinition, ElfSymbolTable, ElfSymbolType};
use plinky_macros::{Display, Error};
use std::collections::{btree_map, BTreeMap};

#[derive(Debug)]
pub(crate) struct Symbols {
    local_symbols: BTreeMap<SymbolId, Symbol>,
    global_symbols_lookup: BTreeMap<SymbolId, Interned<String>>,
    global_symbols: BTreeMap<Interned<String>, Symbol>,
}

impl Symbols {
    pub(crate) fn new() -> Self {
        Self {
            local_symbols: BTreeMap::new(),
            global_symbols_lookup: BTreeMap::new(),
            global_symbols: BTreeMap::new(),
        }
    }

    pub(crate) fn add_unknown_global(&mut self, name: &str) {
        let name = intern(name);
        self.global_symbols.entry(name).or_insert(Symbol {
            name,
            span: intern(ObjectSpan::new_synthetic()),
            visibility: SymbolVisibility::Global { weak: false },
            value: SymbolValue::Undefined,
        });
    }

    pub(crate) fn load_table(
        &mut self,
        span: Interned<ObjectSpan>,
        table: ElfSymbolTable<SerialIds>,
        strings: &Strings,
    ) -> Result<(), LoadSymbolsError> {
        for (symbol_id, elf_symbol) in table.symbols.into_iter() {
            match elf_symbol.type_ {
                ElfSymbolType::NoType => {}
                ElfSymbolType::Object => {}
                ElfSymbolType::Function => {}
                ElfSymbolType::Section => {}
                // The file symbol type is not actually used, so we can omit it.
                ElfSymbolType::File => continue,
                ElfSymbolType::Unknown(_) => {
                    return Err(LoadSymbolsError::UnsupportedUnknownSymbolType)
                }
            }

            let name = intern(
                strings
                    .get(elf_symbol.name)
                    .map_err(|e| LoadSymbolsError::MissingSymbolName(symbol_id, e))?,
            );
            let symbol = Symbol {
                name,
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

            match symbol.visibility {
                SymbolVisibility::Local => {
                    self.local_symbols.insert(symbol_id, symbol);
                }
                SymbolVisibility::Global { weak: false } => {
                    match self.global_symbols.entry(symbol.name) {
                        btree_map::Entry::Vacant(vacant) => {
                            vacant.insert(symbol);
                        }
                        btree_map::Entry::Occupied(mut entry) => {
                            let existing_symbol = entry.get();
                            if let SymbolValue::Undefined = existing_symbol.value {
                                entry.insert(symbol);
                            } else if let SymbolValue::Undefined = symbol.value {
                                // Nothing.
                            } else {
                                return Err(LoadSymbolsError::DuplicateGlobalSymbol(symbol.name));
                            }
                        }
                    }
                    self.global_symbols_lookup.insert(symbol_id, name);
                }
                SymbolVisibility::Global { weak: true } => {
                    todo!("weak symbols are not supported yet")
                }
            }
        }
        Ok(())
    }

    pub(crate) fn get(&self, id: SymbolId) -> Result<&Symbol, MissingGlobalSymbol> {
        if let Some(symbol) = self.local_symbols.get(&id) {
            Ok(symbol)
        } else if let Some(symbol_name) = self.global_symbols_lookup.get(&id) {
            self.get_global(*symbol_name)
        } else {
            panic!("symbol id doesn't point to a symbol");
        }
    }

    pub(crate) fn get_global(
        &self,
        name: Interned<String>,
    ) -> Result<&Symbol, MissingGlobalSymbol> {
        self.global_symbols.get(&name).ok_or(MissingGlobalSymbol { name })
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = &Symbol> {
        self.local_symbols.values().chain(self.global_symbols.values())
    }
}

#[derive(Debug)]
pub(crate) struct Symbol {
    pub(crate) name: Interned<String>,
    pub(crate) span: Interned<ObjectSpan>,
    pub(crate) visibility: SymbolVisibility,
    pub(crate) value: SymbolValue,
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
