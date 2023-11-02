use crate::ids::ElfIds;
use crate::{
    Object, Relocation, RelocationsTable, Section, SectionContent, Symbol, SymbolDefinition,
    SymbolTable,
};
use std::collections::BTreeMap;

pub trait ConvertibleElfIds<F>
where
    F: ElfIds,
    Self: ElfIds + Sized,
{
    fn create_conversion_map(
        &mut self,
        object: &Object<F>,
        string_ids: &[F::StringId],
    ) -> IdConversionMap<F, Self>;
}

pub struct IdConversionMap<F: ElfIds, T: ElfIds> {
    pub section_ids: BTreeMap<F::SectionId, T::SectionId>,
    pub symbol_ids: BTreeMap<F::SymbolId, T::SymbolId>,
    pub string_ids: BTreeMap<F::StringId, T::StringId>,
}

impl<F: ElfIds, T: ElfIds> IdConversionMap<F, T> {
    pub fn new() -> Self {
        Self {
            section_ids: BTreeMap::new(),
            symbol_ids: BTreeMap::new(),
            string_ids: BTreeMap::new(),
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

    fn string_id(&self, old: &F::StringId) -> T::StringId {
        match self.string_ids.get(old) {
            Some(id) => id.clone(),
            None => panic!("bug: string id {old:?} not in conversion map"),
        }
    }
}

pub(crate) fn convert<F, T>(ids: &mut T, object: Object<F>) -> Object<T>
where
    F: ElfIds,
    T: ConvertibleElfIds<F>,
{
    let string_ids = collect_string_ids(&object);
    let map = ids.create_conversion_map(&object, &string_ids);

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
                        name: map.string_id(&section.name),
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
                                                    name: map.string_id(&symbol.name),
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

fn collect_string_ids<I: ElfIds>(object: &Object<I>) -> Vec<I::StringId> {
    let mut ids = Vec::new();
    for section in object.sections.values() {
        ids.push(section.name.clone());
        if let SectionContent::SymbolTable(table) = &section.content {
            for symbol in table.symbols.values() {
                ids.push(symbol.name.clone());
            }
        }
    }
    ids
}
