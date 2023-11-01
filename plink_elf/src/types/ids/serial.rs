use super::convert::IdConversionMap;
use crate::ids::convert::ConvertibleElfIds;
use crate::ids::ElfIds;
use crate::{Object, SectionContent};

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SectionId(usize);

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SymbolId(usize);

#[derive(Debug)]
pub struct SerialIds {
    next_section_id: usize,
    next_symbol_id: usize,
}

impl ElfIds for SerialIds {
    type SectionId = SectionId;
    type SymbolId = SymbolId;
}

impl ConvertibleElfIds for SerialIds {
    fn create_conversion_map<F: ElfIds>(&mut self, object: &Object<F>) -> IdConversionMap<F, Self> {
        let mut map = IdConversionMap::new();

        for (old_id, section) in &object.sections {
            map.section_ids
                .insert(old_id.clone(), self.allocate_section_id());

            match &section.content {
                SectionContent::SymbolTable(table) => {
                    for (id, _) in &table.symbols {
                        map.symbol_ids.insert(id.clone(), self.allocate_symbol_id());
                    }
                }
                _ => {}
            }
        }

        map
    }
}

impl SerialIds {
    pub fn new() -> Self {
        Self {
            next_section_id: 0,
            next_symbol_id: 0,
        }
    }

    fn allocate_section_id(&mut self) -> SectionId {
        let id = SectionId(self.next_section_id);
        self.next_section_id += 1;
        id
    }

    fn allocate_symbol_id(&mut self) -> SymbolId {
        let id = SymbolId(self.next_symbol_id);
        self.next_symbol_id += 1;
        id
    }
}
