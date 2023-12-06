use crate::repr::object::ObjectLoadError;
use crate::repr::strings::Strings;
use plink_elf::ids::serial::{SerialIds, SymbolId};
use plink_elf::{ElfSymbol, ElfSymbolBinding, ElfSymbolDefinition, ElfSymbolTable};
use plink_macros::{Display, Error};
use std::collections::{btree_map, BTreeMap};

#[derive(Debug)]
pub(crate) struct Symbols {
    local_symbols: BTreeMap<SymbolId, ElfSymbol<SerialIds>>,
    global_symbols_map: BTreeMap<SymbolId, String>,
    global_symbols: BTreeMap<String, GlobalSymbol>,
}

impl Symbols {
    pub(crate) fn new() -> Self {
        Self {
            local_symbols: BTreeMap::new(),
            global_symbols_map: BTreeMap::new(),
            global_symbols: BTreeMap::new(),
        }
    }

    pub(crate) fn load_table(
        &mut self,
        table: ElfSymbolTable<SerialIds>,
        strings: &Strings,
    ) -> Result<(), ObjectLoadError> {
        for (symbol_id, symbol) in table.symbols.into_iter() {
            match symbol.binding {
                ElfSymbolBinding::Local => {
                    self.local_symbols.insert(symbol_id, symbol);
                }
                ElfSymbolBinding::Global => {
                    let name = strings
                        .get(symbol.name)
                        .map_err(|e| ObjectLoadError::MissingSymbolName(symbol_id, e))?
                        .to_string();

                    let symbol = match symbol.definition {
                        ElfSymbolDefinition::Undefined => GlobalSymbol::Undefined,
                        _ => GlobalSymbol::Strong(symbol),
                    };

                    match self.global_symbols.entry(name.clone()) {
                        btree_map::Entry::Vacant(vacant) => {
                            vacant.insert(symbol);
                        }
                        btree_map::Entry::Occupied(mut existing_symbol) => {
                            match (existing_symbol.get(), &symbol) {
                                (GlobalSymbol::Strong(_), GlobalSymbol::Strong(_)) => {
                                    return Err(ObjectLoadError::DuplicateGlobalSymbol(name));
                                }
                                (GlobalSymbol::Strong(_), GlobalSymbol::Undefined) => {}
                                (GlobalSymbol::Undefined, _) => {
                                    existing_symbol.insert(symbol);
                                }
                            }
                        }
                    }
                    self.global_symbols_map.insert(symbol_id, name);
                }
                ElfSymbolBinding::Weak => todo!("weak symbols are not supported yet"),
                ElfSymbolBinding::Unknown(_) => {
                    return Err(ObjectLoadError::UnsupportedUnknownSymbolBinding);
                }
            }
        }
        Ok(())
    }

    pub(crate) fn get(&self, id: SymbolId) -> Result<&ElfSymbol<SerialIds>, MissingGlobalSymbol> {
        if let Some(symbol) = self.local_symbols.get(&id) {
            Ok(symbol)
        } else if let Some(symbol_name) = self.global_symbols_map.get(&id) {
            self.get_global(symbol_name)
        } else {
            panic!("symbol id doesn't point to a symbol");
        }
    }

    pub(crate) fn get_global(
        &self,
        name: &str,
    ) -> Result<&ElfSymbol<SerialIds>, MissingGlobalSymbol> {
        match self.global_symbols.get(name) {
            Some(GlobalSymbol::Strong(symbol)) => Ok(symbol),
            Some(GlobalSymbol::Undefined) | None => Err(MissingGlobalSymbol(name.into())),
        }
    }
}

#[derive(Debug)]
enum GlobalSymbol {
    Strong(ElfSymbol<SerialIds>),
    Undefined,
}

#[derive(Debug, Error, Display)]
#[display("missing global symbol: {f0}")]
pub(crate) struct MissingGlobalSymbol(String);
