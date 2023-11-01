use crate::ids::ElfIds;
use crate::Object;
use std::collections::BTreeMap;

pub trait ConvertibleElfIds: ElfIds {
    fn create_conversion_map<F>(&mut self, object: &Object<F>) -> IdConversionMap<F, Self>
    where
        F: ElfIds,
        Self: Sized;
}

pub struct IdConversionMap<F: ElfIds, T: ElfIds> {
    pub section_ids: BTreeMap<F::SectionId, T::SectionId>,
}

impl<F: ElfIds, T: ElfIds> IdConversionMap<F, T> {
    pub fn new() -> Self {
        Self {
            section_ids: BTreeMap::new(),
        }
    }

    fn section_id(&self, old: &F::SectionId) -> T::SectionId {
        match self.section_ids.get(old) {
            Some(id) => id.clone(),
            None => panic!("bug: section id {old:?} not in conversion map"),
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
            .map(|(id, section)| (map.section_id(&id), section))
            .collect(),
        segments: object.segments,
    }
}
