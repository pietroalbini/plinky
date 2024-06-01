use crate::ids::ElfIds;
use crate::{
    ElfGroup, ElfHash, ElfObject, ElfRelocation, ElfRelocationsTable, ElfSection,
    ElfSectionContent, ElfSegment, ElfSegmentContent, ElfSymbol, ElfSymbolDefinition,
    ElfSymbolTable,
};
use std::collections::BTreeMap;

pub trait ConvertibleElfIds<F>
where
    F: ElfIds,
    Self: ElfIds + Sized,
{
    fn create_conversion_map(
        &mut self,
        object: &ElfObject<F>,
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

pub fn convert<F, T>(ids: &mut T, object: ElfObject<F>) -> ElfObject<T>
where
    F: ElfIds,
    T: ConvertibleElfIds<F>,
{
    let string_ids = collect_string_ids(&object);
    let map = ids.create_conversion_map(&object, &string_ids);

    ElfObject {
        env: object.env,
        type_: object.type_,
        entry: object.entry,
        sections: object
            .sections
            .into_iter()
            .map(|(id, section)| {
                (
                    map.section_id(&id),
                    ElfSection {
                        name: map.string_id(&section.name),
                        memory_address: section.memory_address,
                        part_of_group: section.part_of_group,
                        content: match section.content {
                            ElfSectionContent::Null => ElfSectionContent::Null,
                            ElfSectionContent::Program(p) => ElfSectionContent::Program(p),
                            ElfSectionContent::Uninitialized(u) => {
                                ElfSectionContent::Uninitialized(u)
                            }
                            ElfSectionContent::SymbolTable(table) => {
                                ElfSectionContent::SymbolTable(ElfSymbolTable {
                                    symbols: table
                                        .symbols
                                        .into_iter()
                                        .map(|(id, symbol)| {
                                            (
                                                map.symbol_id(&id),
                                                ElfSymbol {
                                                    name: map.string_id(&symbol.name),
                                                    binding: symbol.binding,
                                                    type_: symbol.type_,
                                                    visibility: symbol.visibility,
                                                    definition: match symbol.definition {
                                                        ElfSymbolDefinition::Undefined => {
                                                            ElfSymbolDefinition::Undefined
                                                        }
                                                        ElfSymbolDefinition::Absolute => {
                                                            ElfSymbolDefinition::Absolute
                                                        }
                                                        ElfSymbolDefinition::Common => {
                                                            ElfSymbolDefinition::Common
                                                        }
                                                        ElfSymbolDefinition::Section(
                                                            section_id,
                                                        ) => ElfSymbolDefinition::Section(
                                                            map.section_id(&section_id),
                                                        ),
                                                    },
                                                    value: symbol.value,
                                                    size: symbol.size,
                                                },
                                            )
                                        })
                                        .collect(),
                                })
                            }
                            ElfSectionContent::StringTable(s) => ElfSectionContent::StringTable(s),
                            ElfSectionContent::RelocationsTable(table) => {
                                ElfSectionContent::RelocationsTable(ElfRelocationsTable {
                                    symbol_table: map.section_id(&table.symbol_table),
                                    applies_to_section: map.section_id(&table.applies_to_section),
                                    relocations: table
                                        .relocations
                                        .into_iter()
                                        .map(|relocation| ElfRelocation {
                                            offset: relocation.offset,
                                            symbol: map.symbol_id(&relocation.symbol),
                                            relocation_type: relocation.relocation_type,
                                            addend: relocation.addend,
                                        })
                                        .collect(),
                                })
                            }
                            ElfSectionContent::Group(g) => ElfSectionContent::Group(ElfGroup {
                                symbol_table: map.section_id(&g.symbol_table),
                                signature: map.symbol_id(&g.signature),
                                sections: g.sections.iter().map(|s| map.section_id(s)).collect(),
                                comdat: g.comdat,
                            }),
                            ElfSectionContent::Hash(h) => ElfSectionContent::Hash(ElfHash {
                                symbol_table: map.section_id(&h.symbol_table),
                                buckets: h.buckets,
                                chain: h.chain,
                            }),
                            ElfSectionContent::Note(n) => ElfSectionContent::Note(n),
                            ElfSectionContent::Unknown(u) => ElfSectionContent::Unknown(u),
                        },
                    },
                )
            })
            .collect(),
        segments: object
            .segments
            .into_iter()
            .map(|segment| ElfSegment {
                type_: segment.type_,
                perms: segment.perms,
                content: match segment.content {
                    ElfSegmentContent::Empty => ElfSegmentContent::Empty,
                    ElfSegmentContent::Unknown(unknown) => ElfSegmentContent::Unknown(unknown),
                    ElfSegmentContent::Sections(ids) => ElfSegmentContent::Sections(
                        ids.into_iter().map(|id| map.section_id(&id)).collect(),
                    ),
                },
                align: segment.align,
            })
            .collect(),
    }
}

fn collect_string_ids<I: ElfIds>(object: &ElfObject<I>) -> Vec<I::StringId> {
    let mut ids = Vec::new();
    for section in object.sections.values() {
        ids.push(section.name.clone());
        if let ElfSectionContent::SymbolTable(table) = &section.content {
            for symbol in table.symbols.values() {
                ids.push(symbol.name.clone());
            }
        }
    }
    ids
}
