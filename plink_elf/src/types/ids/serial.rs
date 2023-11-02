use super::convert::IdConversionMap;
use crate::ids::convert::ConvertibleElfIds;
use crate::ids::{ElfIds, StringIdGetters};
use crate::{Object, SectionContent};

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SectionId(usize);

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SymbolId(usize);

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StringId(SectionId, u32);

impl StringIdGetters<SerialIds> for StringId {
    fn section(&self) -> &SectionId {
        &self.0
    }

    fn offset(&self) -> u32 {
        self.1
    }
}

#[derive(Debug)]
pub struct SerialIds {
    next_section_id: usize,
    next_symbol_id: usize,
}

impl ElfIds for SerialIds {
    type SectionId = SectionId;
    type SymbolId = SymbolId;
    type StringId = StringId;
}

impl<F> ConvertibleElfIds<F> for SerialIds
where
    F: ElfIds,
    F::StringId: StringIdGetters<F>,
{
    fn create_conversion_map(
        &mut self,
        object: &Object<F>,
        string_ids: &[F::StringId],
    ) -> IdConversionMap<F, Self> {
        let mut map = IdConversionMap::<F, Self>::new();

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

        for string_id in string_ids {
            map.string_ids.insert(
                string_id.clone(),
                StringId(
                    *map.section_ids
                        .get(string_id.section())
                        .expect("missing section"),
                    string_id.offset(),
                ),
            );
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
