use crate::linker::object::ObjectLoadError;
use crate::linker::strings::Strings;
use plink_elf::ids::serial::{SerialIds, SymbolId};
use plink_elf::{ElfSymbol, ElfSymbolBinding, ElfSymbolDefinition, ElfSymbolTable};
use std::collections::{btree_map, BTreeMap};

#[derive(Debug)]
pub(super) struct Symbols {
    local_symbols: BTreeMap<SymbolId, ElfSymbol<SerialIds>>,
    global_symbols_map: BTreeMap<SymbolId, String>,
    global_symbols: BTreeMap<String, GlobalSymbol>,
}

impl Symbols {
    pub(super) fn new() -> Self {
        Self {
            local_symbols: BTreeMap::new(),
            global_symbols_map: BTreeMap::new(),
            global_symbols: BTreeMap::new(),
        }
    }

    pub(super) fn load_table(
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
}

#[derive(Debug)]
enum GlobalSymbol {
    Strong(ElfSymbol<SerialIds>),
    Undefined,
}
