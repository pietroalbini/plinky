use crate::interner::{intern, Interned};
use crate::repr::relocations::Relocation;
use crate::repr::symbols::views::{AllSymbols, SymbolsView};
use crate::repr::symbols::{SymbolValue, Symbols};
use crate::utils::before_freeze::BeforeFreeze;
use plinky_diagnostics::ObjectSpan;
use plinky_elf::ids::serial::{SectionId, SerialIds};
use plinky_elf::{ElfDeduplication, ElfPermissions};
use plinky_macros::Getters;
use std::collections::BTreeMap;

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

    pub(crate) fn builder<'a>(
        &'a mut self,
        name: &str,
        content: impl Into<SectionContent>,
    ) -> SectionBuilder<'a> {
        SectionBuilder {
            parent: self,
            name: intern(name),
            content: content.into(),
            source: ObjectSpan::new_synthetic(),
        }
    }

    pub(crate) fn remove(
        &mut self,
        id: SectionId,
        purge_symbols_from: Option<&mut Symbols>,
        before_freeze: &BeforeFreeze,
    ) -> Option<Section> {
        let removed_section = self.inner.remove(&id)?;
        self.names_of_removed_sections.insert(id, removed_section.name);

        if let Some(symbols) = purge_symbols_from {
            let mut symbols_to_remove = Vec::new();
            for (symbol_id, symbol) in symbols.iter(&AllSymbols) {
                let SymbolValue::SectionRelative { section, .. } = symbol.value() else {
                    continue;
                };
                if section == removed_section.id {
                    symbols_to_remove.push(symbol_id);
                }
            }
            for symbol_id in symbols_to_remove {
                symbols.remove(symbol_id, before_freeze);
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

#[must_use]
pub(crate) struct SectionBuilder<'a> {
    parent: &'a mut Sections,
    name: Interned<String>,
    content: SectionContent,
    source: ObjectSpan,
}

impl SectionBuilder<'_> {
    pub(crate) fn source(mut self, source: ObjectSpan) -> Self {
        self.source = source;
        self
    }

    pub(crate) fn create(self, ids: &mut SerialIds) -> SectionId {
        let id = ids.allocate_section_id();
        self.create_with_id(id);
        id
    }

    pub(crate) fn create_with_id(self, id: SectionId) {
        // Avoid stale data if the section was removed and then added again.
        self.parent.names_of_removed_sections.remove(&id);

        self.parent.inner.insert(
            id,
            Section {
                id,
                name: self.name,
                source: self.source,
                content: self.content,
                _prevent_creation: (),
            },
        );
    }
}

#[derive(Debug)]
pub(crate) struct Section {
    pub(crate) id: SectionId,
    pub(crate) name: Interned<String>,
    pub(crate) source: ObjectSpan,
    pub(crate) content: SectionContent,
    _prevent_creation: (),
}

#[derive(Debug)]
pub(crate) enum SectionContent {
    Data(DataSection),
    Uninitialized(UninitializedSection),
    StringsForSymbols(StringsForSymbolsSection),
    Symbols(SymbolsSection),
    SysvHash(SysvHashSection),
    Relocations(RelocationsSection),
}

#[derive(Debug)]
pub(crate) struct DataSection {
    pub(crate) perms: ElfPermissions,
    pub(crate) deduplication: ElfDeduplication,
    pub(crate) bytes: Vec<u8>,
    pub(crate) relocations: Vec<Relocation>,
}

impl DataSection {
    pub(crate) fn new(perms: ElfPermissions, content: &[u8]) -> Self {
        Self {
            perms,
            deduplication: ElfDeduplication::Disabled,
            bytes: content.into(),
            relocations: Vec::new(),
        }
    }
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

impl StringsForSymbolsSection {
    pub(crate) fn new(view: impl SymbolsView + 'static) -> Self {
        Self { view: Box::new(view) }
    }
}

#[derive(Debug)]
pub(crate) struct SymbolsSection {
    pub(crate) strings: SectionId,
    pub(crate) is_dynamic: bool,
    pub(crate) view: Box<dyn SymbolsView>,
}

impl SymbolsSection {
    pub(crate) fn new(
        strings: SectionId,
        view: impl SymbolsView + 'static,
        is_dynamic: bool,
    ) -> Self {
        Self { strings, view: Box::new(view), is_dynamic }
    }
}

#[derive(Debug)]
pub(crate) struct SysvHashSection {
    pub(crate) view: Box<dyn SymbolsView>,
    pub(crate) symbols: SectionId,
}

impl SysvHashSection {
    pub(crate) fn new(view: impl SymbolsView + 'static, symbols: SectionId) -> Self {
        Self { view: Box::new(view), symbols }
    }
}

#[derive(Debug, Getters)]
pub(crate) struct RelocationsSection {
    #[get]
    section: Option<SectionId>,
    #[get]
    symbols_table: SectionId,
    relocations: Vec<Relocation>,
}

impl RelocationsSection {
    pub(crate) fn new(
        section: Option<SectionId>,
        symbols_table: SectionId,
        relocations: Vec<Relocation>,
    ) -> Self {
        Self { section, symbols_table, relocations }
    }

    pub(crate) fn relocations(&self) -> &[Relocation] {
        &self.relocations
    }
}

macro_rules! from {
    (impl From<$from:ident> for $enum:ident::$variant:ident) => {
        impl From<$from> for $enum {
            fn from(value: $from) -> $enum {
                $enum::$variant(value)
            }
        }
    };
}

from!(impl From<DataSection> for SectionContent::Data);
from!(impl From<UninitializedSection> for SectionContent::Uninitialized);
from!(impl From<StringsForSymbolsSection> for SectionContent::StringsForSymbols);
from!(impl From<SymbolsSection> for SectionContent::Symbols);
from!(impl From<SysvHashSection> for SectionContent::SysvHash);
from!(impl From<RelocationsSection> for SectionContent::Relocations);
