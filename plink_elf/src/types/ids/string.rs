use super::IdConversionMap;
use crate::ids::{ConvertibleElfIds, ElfIds};
use crate::Object;

#[derive(Debug)]
pub struct StringIds(());

impl StringIds {
    pub fn new() -> Self {
        Self(())
    }
}

impl ElfIds for StringIds {
    type SectionId = String;
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
        }

        map
    }
}
