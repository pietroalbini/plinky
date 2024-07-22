use crate::interner::Interned;
use crate::repr::relocations::Relocation;
use crate::repr::symbols::{SymbolValue, Symbols};
use plinky_diagnostics::ObjectSpan;
use plinky_elf::ids::serial::SectionId;
use plinky_elf::{ElfDeduplication, ElfPermissions};
use std::collections::BTreeMap;
use crate::repr::symbols::views::{AllSymbols, SymbolsView};

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

    pub(crate) fn remove(
        &mut self,
        id: SectionId,
        purge_symbols_from: Option<&mut Symbols>,
    ) -> Option<Section> {
        let removed_section = self.inner.remove(&id)?;
        self.names_of_removed_sections.insert(id, removed_section.name);

        if let Some(symbols) = purge_symbols_from {
            let mut symbols_to_remove = Vec::new();
            for (symbol_id, symbol) in symbols.iter(&AllSymbols) {
                let SymbolValue::SectionRelative { section, .. } = &symbol.value else {
                    continue;
                };
                if *section == removed_section.id {
                    symbols_to_remove.push(symbol_id);
                }
            }
            for symbol_id in symbols_to_remove {
                symbols.remove(symbol_id);
            }
        }

        Some(removed_section)
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
    pub(crate) source: ObjectSpan,
    pub(crate) content: SectionContent,
}

#[derive(Debug)]
pub(crate) enum SectionContent {
    Data(DataSection),
    Uninitialized(UninitializedSection),
    StringsForSymbols(StringsForSymbolsSection),
    Symbols(SymbolsSection),
}

#[derive(Debug)]
pub(crate) struct DataSection {
    pub(crate) perms: ElfPermissions,
    pub(crate) deduplication: ElfDeduplication,
    pub(crate) bytes: Vec<u8>,
    pub(crate) relocations: Vec<Relocation>,
}

#[derive(Debug)]
pub(crate) struct UninitializedSection {
    pub(crate) perms: ElfPermissions,
    pub(crate) len: u64,
}

#[derive(Debug)]
pub(crate) struct StringsForSymbolsSection {
    pub(crate) view: Box<dyn SymbolsView>,
}

#[derive(Debug)]
pub(crate) struct SymbolsSection {
    pub(crate) strings: SectionId,
    pub(crate) view: Box<dyn SymbolsView>,
}
