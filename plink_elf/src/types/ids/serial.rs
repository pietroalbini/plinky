use crate::ids::ElfIds;
use crate::Object;
use crate::ids::convert::ConvertibleElfIds;
use super::convert::IdConversionMap;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SectionId(usize);

#[derive(Debug)]
pub struct SerialIds {
    next_section_id: usize,
}

impl ElfIds for SerialIds {
    type SectionId = SectionId;
}

impl ConvertibleElfIds for SerialIds {
    fn create_conversion_map<F: ElfIds>(&mut self, object: &Object<F>) -> IdConversionMap<F, Self> {
        let mut map = IdConversionMap::new();

        for (old_id, _) in &object.sections {
            map.section_ids.insert(old_id.clone(), self.allocate_section_id());
        }

        map
    }
}

impl SerialIds {
    pub fn new() -> Self {
        Self { next_section_id: 0 }
    }

    fn allocate_section_id(&mut self) -> SectionId {
        let id = SectionId(self.next_section_id);
        self.next_section_id += 1;
        id
    }
}
