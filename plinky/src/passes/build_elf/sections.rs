use crate::passes::build_elf::ids::{BuiltElfIds, BuiltElfSectionId};
use crate::passes::build_elf::PendingStringsTable;
use crate::passes::layout::SectionLayout;
use plinky_elf::ids::serial::SectionId;
use plinky_elf::{ElfSection, ElfSectionContent};
use std::collections::BTreeMap;

pub(super) struct Sections {
    sections: BTreeMap<BuiltElfSectionId, ElfSection<BuiltElfIds>>,
    ids_map: BTreeMap<SectionId, BuiltElfSectionId>,
    names: PendingStringsTable,
}

impl Sections {
    pub(super) fn new(ids: &mut BuiltElfIds) -> Self {
        let zero_id = ids.allocate_section_id();

        let names = PendingStringsTable::new(ids);
        let mut sections = BTreeMap::new();

        // The first section must always be the null section.
        sections.insert(
            zero_id,
            ElfSection {
                name: names.zero_id,
                memory_address: 0,
                part_of_group: false,
                content: ElfSectionContent::Null,
            },
        );

        Sections { sections, ids_map: BTreeMap::new(), names }
    }

    pub(super) fn create<'a>(
        &'a mut self,
        name: &'a str,
        content: ElfSectionContent<BuiltElfIds>,
    ) -> SectionBuilder<'a> {
        SectionBuilder { parent: self, name, content, memory_address: 0, old_id: None }
    }

    pub(super) fn new_id_of(&self, old_id: SectionId) -> BuiltElfSectionId {
        *self.ids_map.get(&old_id).expect("could not convert section ids")
    }

    pub(super) fn finalize(mut self) -> BTreeMap<BuiltElfSectionId, ElfSection<BuiltElfIds>> {
        let shstrtab = self.names.add(".shstrtab");
        self.sections.insert(
            self.names.id,
            ElfSection {
                name: shstrtab,
                memory_address: 0,
                part_of_group: false,
                content: self.names.into_elf(),
            },
        );
        self.sections
    }
}

#[must_use]
pub(super) struct SectionBuilder<'a> {
    parent: &'a mut Sections,
    name: &'a str,
    content: ElfSectionContent<BuiltElfIds>,
    memory_address: u64,
    old_id: Option<SectionId>,
}

impl SectionBuilder<'_> {
    pub(super) fn layout(mut self, layout: &SectionLayout) -> Self {
        match layout {
            SectionLayout::Allocated { address } => self.memory_address = *address,
            SectionLayout::NotAllocated => {}
        }
        self
    }

    pub(super) fn old_id(mut self, id: SectionId) -> Self {
        self.old_id = Some(id);
        self
    }

    pub(super) fn add(self, ids: &mut BuiltElfIds) {
        self.add_with_id(ids.allocate_section_id())
    }

    pub(super) fn add_with_id(self, id: BuiltElfSectionId) {
        self.parent.sections.insert(
            id,
            ElfSection {
                name: self.parent.names.add(self.name),
                memory_address: self.memory_address,
                part_of_group: false,
                content: self.content,
            },
        );
        if let Some(old) = self.old_id {
            self.parent.ids_map.insert(old, id);
        }
    }
}
