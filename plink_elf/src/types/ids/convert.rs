use crate::ids::ElfIds;
use crate::{
    Object, Relocation, RelocationsTable, Section, SectionContent, Symbol, SymbolDefinition,
    SymbolTable,
};
use std::collections::BTreeMap;

pub trait ConvertibleElfIds: ElfIds {
    fn create_conversion_map<F>(&mut self, object: &Object<F>) -> IdConversionMap<F, Self>
    where
        F: ElfIds,
        Self: Sized;
}

pub struct IdConversionMap<F: ElfIds, T: ElfIds> {
    pub section_ids: BTreeMap<F::SectionId, T::SectionId>,
    pub symbol_ids: BTreeMap<F::SymbolId, T::SymbolId>,
}

impl<F: ElfIds, T: ElfIds> IdConversionMap<F, T> {
    pub fn new() -> Self {
        Self {
            section_ids: BTreeMap::new(),
            symbol_ids: BTreeMap::new(),
        }
    }

    fn section_id(&self, old: &F::SectionId) -> T::SectionId {
        match self.section_ids.get(old) {
            Some(id) => id.clone(),
            None => panic!("bug: section id {old:?} not in conversion map"),
        }
    }

    fn symbol_id(&self, old: &F::SymbolId) -> T::SymbolId {
        match self.symbol_ids.get(old) {
            Some(id) => id.clone(),
            None => panic!("bug: symbol id {old:?} not in conversion map"),
        }
    }
}

pub(crate) fn convert<F, T>(ids: &mut T, object: Object<F>) -> Object<T>
where
    F: ElfIds,
    T: ConvertibleElfIds,
{
    let map = ids.create_conversion_map(&object);

    Object {
        env: object.env,
        type_: object.type_,
        entry: object.entry,
        flags: object.flags,
        sections: object
            .sections
            .into_iter()
            .map(|(id, section)| {
                (
                    map.section_id(&id),
                    Section {
                        name: section.name,
                        memory_address: section.memory_address,
                        content: match section.content {
                            SectionContent::Null => SectionContent::Null,
                            SectionContent::Program(p) => SectionContent::Program(p),
                            SectionContent::SymbolTable(table) => {
                                SectionContent::SymbolTable(SymbolTable {
                                    symbols: table
                                        .symbols
                                        .into_iter()
                                        .map(|(id, symbol)| {
                                            (
                                                map.symbol_id(&id),
                                                Symbol {
                                                    name: symbol.name,
                                                    binding: symbol.binding,
                                                    type_: symbol.type_,
                                                    definition: match symbol.definition {
                                                        SymbolDefinition::Undefined => {
                                                            SymbolDefinition::Undefined
                                                        }
                                                        SymbolDefinition::Absolute => {
                                                            SymbolDefinition::Absolute
                                                        }
                                                        SymbolDefinition::Common => {
                                                            SymbolDefinition::Common
                                                        }
                                                        SymbolDefinition::Section(section_id) => {
                                                            SymbolDefinition::Section(
                                                                map.section_id(&section_id),
                                                            )
                                                        }
                                                    },
                                                    value: symbol.value,
                                                    size: symbol.size,
                                                },
                                            )
                                        })
                                        .collect(),
                                })
                            }
                            SectionContent::StringTable(s) => SectionContent::StringTable(s),
                            SectionContent::RelocationsTable(table) => {
                                SectionContent::RelocationsTable(RelocationsTable {
                                    symbol_table: map.section_id(&table.symbol_table),
                                    applies_to_section: map.section_id(&table.applies_to_section),
                                    relocations: table
                                        .relocations
                                        .into_iter()
                                        .map(|relocation| Relocation {
                                            offset: relocation.offset,
                                            symbol: map.symbol_id(&relocation.symbol),
                                            relocation_type: relocation.relocation_type,
                                            addend: relocation.addend,
                                        })
                                        .collect(),
                                })
                            }
                            SectionContent::Note(n) => SectionContent::Note(n),
                            SectionContent::Unknown(u) => SectionContent::Unknown(u),
                        },
                    },
                )
            })
            .collect(),
        segments: object.segments,
    }
}
