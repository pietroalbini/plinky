use crate::interner::Interned;
use crate::repr::strings::Strings;
use crate::repr::symbols::Symbols;
use plinky_diagnostics::ObjectSpan;
use plinky_elf::ids::serial::{SectionId, SerialIds, SymbolId};
use plinky_elf::{ElfDeduplication, ElfEnvironment, ElfPermissions, ElfRelocation};
use std::collections::BTreeMap;

#[derive(Debug)]
pub(crate) struct Object {
    pub(crate) env: ElfEnvironment,
    pub(crate) sections: Sections,
    pub(crate) strings: Strings,
    pub(crate) symbols: Symbols,
    pub(crate) entry_point: SymbolId,
    pub(crate) executable_stack: bool,
}

#[derive(Debug)]
pub(crate) struct Sections {
    inner: BTreeMap<SectionId, Section>,
    names_of_removed_sections: BTreeMap<SectionId, Interned<String>>,
}

impl Sections {
    pub(crate) fn new() -> Self {
        Self { inner: BTreeMap::new(), names_of_removed_sections: BTreeMap::new() }
    }

    pub(crate) fn get(&self, id: SectionId) -> Option<&Section> {
        self.inner.get(&id)
    }

    pub(crate) fn add(&mut self, section: Section) {
        // Avoid stale data if the section was removed and then added again.
        self.names_of_removed_sections.remove(&section.id);

        self.inner.insert(section.id, section);
    }

    pub(crate) fn remove(&mut self, id: SectionId) {
        if let Some(section) = self.inner.remove(&id) {
            self.names_of_removed_sections.insert(id, section.name);
        }
    }

    pub(crate) fn pop_first(&mut self) -> Option<Section> {
        self.inner.pop_first().map(|(_id, section)| section)
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = &Section> {
        self.inner.values()
    }

    pub(crate) fn iter_mut(&mut self) -> impl Iterator<Item = &mut Section> {
        self.inner.values_mut()
    }

    pub(crate) fn name_of_removed_section(&self, id: SectionId) -> Option<Interned<String>> {
        self.names_of_removed_sections.get(&id).copied()
    }
}

#[derive(Debug)]
pub(crate) struct Section {
    pub(crate) id: SectionId,
    pub(crate) name: Interned<String>,
    pub(crate) perms: ElfPermissions,
    pub(crate) source: ObjectSpan,
    pub(crate) content: SectionContent,
}

#[derive(Debug)]
pub(crate) enum SectionContent {
    Data(DataSection),
    Uninitialized(UninitializedSection),
}

#[derive(Debug)]
pub(crate) struct DataSection {
    pub(crate) deduplication: ElfDeduplication,
    pub(crate) bytes: Vec<u8>,
    pub(crate) relocations: Vec<ElfRelocation<SerialIds>>,
}

#[derive(Debug)]
pub(crate) struct UninitializedSection {
    pub(crate) len: u64,
}
