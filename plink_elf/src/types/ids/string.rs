use super::IdConversionMap;
use crate::ids::{ConvertibleElfIds, ElfIds};
use crate::{Object, SectionContent};

#[derive(Debug)]
pub struct StringIds(());

impl StringIds {
    pub fn new() -> Self {
        Self(())
    }
}

impl ElfIds for StringIds {
    type SectionId = String;
    type SymbolId = String;
}

impl ConvertibleElfIds for StringIds {
    fn create_conversion_map<F>(&mut self, object: &Object<F>) -> IdConversionMap<F, Self>
    where
        F: ElfIds,
        Self: Sized,
    {
        let mut map = IdConversionMap::new();

        for (id, section) in &object.sections {
            map.section_ids.insert(id.clone(), section.name.clone());

            match &section.content {
                SectionContent::SymbolTable(table) => {
                    for (id, symbol) in &table.symbols {
                        map.symbol_ids.insert(id.clone(), symbol.name.clone());
                    }
                }
                _ => {}
            }
        }

        map
    }
}
