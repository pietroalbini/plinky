use crate::interner::{Interned, intern};
use crate::repr::relocations::Relocation;
use crate::repr::symbols::views::{AllSymbols, SymbolsView};
use crate::repr::symbols::{SymbolValue, Symbols};
use plinky_diagnostics::ObjectSpan;
use plinky_elf::{ElfClass, ElfDeduplication, ElfNote, ElfPermissions};
use plinky_macros::Getters;
use plinky_utils::ints::Length;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Debug)]
pub(crate) struct Sections {
    inner: VecDeque<SectionSlot>,
}

impl Sections {
    pub(crate) fn new() -> Self {
        Self { inner: VecDeque::new() }
    }

    pub(crate) fn get(&self, id: SectionId) -> &Section {
        match self.inner.get(id.0) {
            Some(SectionSlot::Present(section)) => section,
            Some(SectionSlot::Removed(removed)) => panic!("section {} was removed", removed.name),
            Some(SectionSlot::Placeholder) => panic!("section is a placeholder"),
            None => panic!("missing section"),
        }
    }

    pub(crate) fn get_mut(&mut self, id: SectionId) -> &mut Section {
        match self.inner.get_mut(id.0) {
            Some(SectionSlot::Present(section)) => section,
            Some(SectionSlot::Removed(removed)) => panic!("section {} was removed", removed.name),
            Some(SectionSlot::Placeholder) => panic!("section is a placeholder"),
            None => panic!("missing section"),
        }
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

    pub(crate) fn reserve_placeholder(&mut self) -> SectionId {
        let id = SectionId(self.inner.len());
        self.inner.push_back(SectionSlot::Placeholder);
        id
    }

    pub(crate) fn remove(
        &mut self,
        id: SectionId,
        purge_symbols_from: Option<&mut Symbols>,
    ) -> Section {
        let old = match self.inner.get_mut(id.0) {
            Some(SectionSlot::Present(section)) => {
                let name = section.name;
                let SectionSlot::Present(old) = std::mem::replace(
                    &mut self.inner[id.0],
                    SectionSlot::Removed(RemovedSection { name }),
                ) else {
                    unreachable!()
                };
                old
            }
            Some(SectionSlot::Removed(section)) => {
                panic!("section {} was already removed", section.name)
            }
            Some(SectionSlot::Placeholder) => panic!("section is a placeholder"),
            None => panic!("missing section"),
        };

        if let Some(symbols) = purge_symbols_from {
            let mut symbols_to_remove = Vec::new();
            for symbol in symbols.iter(&AllSymbols) {
                let section = match symbol.value() {
                    SymbolValue::Section { section } => section,
                    SymbolValue::SectionRelative { section, .. } => section,
                    SymbolValue::SectionVirtualAddress { section, .. } => section,
                    SymbolValue::Absolute { .. }
                    | SymbolValue::SectionNotLoaded
                    | SymbolValue::ExternallyDefined
                    | SymbolValue::Undefined
                    | SymbolValue::Null => continue,
                };
                if section == id {
                    symbols_to_remove.push(symbol.id());
                }
            }
            for symbol_id in symbols_to_remove {
                symbols.remove(symbol_id);
            }
        }

        old
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = &Section> {
        self.inner.iter().filter_map(|slot| match slot {
            SectionSlot::Present(section) => Some(section),
            SectionSlot::Removed(_) => None,
            SectionSlot::Placeholder => None,
        })
    }

    pub(crate) fn iter_mut(&mut self) -> impl Iterator<Item = &mut Section> {
        self.inner.iter_mut().filter_map(|slot| match slot {
            SectionSlot::Present(section) => Some(section),
            SectionSlot::Removed(_) => None,
            SectionSlot::Placeholder => None,
        })
    }

    pub(crate) fn len(&self) -> usize {
        self.inner.iter().filter(|slot| matches!(slot, SectionSlot::Present(_))).count()
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

    pub(crate) fn create(self) -> SectionId {
        let id = SectionId(self.parent.inner.len());
        self.parent.inner.push_back(SectionSlot::Present(Section {
            id,
            name: self.name,
            source: self.source,
            content: self.content,
            _prevent_creation: (),
        }));
        id
    }

    pub(crate) fn create_in_placeholder(self, placeholder: SectionId) -> SectionId {
        match self.parent.inner.get_mut(placeholder.0) {
            Some(SectionSlot::Present(_)) => panic!("a section already exists at this ID"),
            Some(SectionSlot::Removed(_)) => {
                panic!("a section with this ID was previously removed")
            }
            Some(slot @ SectionSlot::Placeholder) => {
                *slot = SectionSlot::Present(Section {
                    id: placeholder,
                    name: self.name,
                    source: self.source,
                    content: self.content,
                    _prevent_creation: (),
                });
                placeholder
            }
            None => panic!("missing placeholder"),
        }
    }
}

#[derive(Debug)]
enum SectionSlot {
    Present(Section),
    Removed(RemovedSection),
    Placeholder,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct SectionId(usize);

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
    Strings(StringsSection),
    Symbols(SymbolsSection),
    SysvHash(SysvHashSection),
    GnuHash(GnuHashSection),
    Relocations(RelocationsSection),
    Dynamic(DynamicSection),
    Notes(NotesSection),
    SectionNames,
}

#[derive(Debug)]
pub(crate) struct DataSection {
    pub(crate) perms: ElfPermissions,
    pub(crate) deduplication: ElfDeduplication,
    pub(crate) bytes: Vec<u8>,
    pub(crate) relocations: Vec<Relocation>,
    pub(crate) inside_relro: bool,
}

impl DataSection {
    pub(crate) fn new(perms: ElfPermissions, content: &[u8]) -> Self {
        Self {
            perms,
            deduplication: ElfDeduplication::Disabled,
            bytes: content.into(),
            relocations: Vec::new(),
            inside_relro: false,
        }
    }
}

#[derive(Debug)]
pub(crate) struct UninitializedSection {
    pub(crate) perms: ElfPermissions,
    pub(crate) len: Length,
}

#[derive(Debug)]
pub(crate) struct StringsSection {
    symbol_names: Box<dyn SymbolsView>,
    custom_strings: Vec<String>,
    generation: usize,
}

impl StringsSection {
    pub(crate) fn new(view: impl SymbolsView + 'static) -> Self {
        static GENERATIONS: AtomicUsize = AtomicUsize::new(0);
        Self {
            symbol_names: Box::new(view),
            custom_strings: Vec::new(),
            generation: GENERATIONS.fetch_add(1, Ordering::Relaxed),
        }
    }

    pub(crate) fn add_custom_string(&mut self, string: impl Into<String>) -> UpcomingStringId {
        let index = self.custom_strings.len();
        self.custom_strings.push(string.into());
        UpcomingStringId { generation: self.generation, index }
    }

    pub(crate) fn iter_custom_strings(&self) -> impl Iterator<Item = (UpcomingStringId, &str)> {
        self.custom_strings.iter().enumerate().map(|(index, string)| {
            (UpcomingStringId { generation: self.generation, index }, string.as_str())
        })
    }

    pub(crate) fn symbol_names_view(&self) -> &dyn SymbolsView {
        &*self.symbol_names
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct UpcomingStringId {
    generation: usize,
    index: usize,
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

#[derive(Debug)]
pub(crate) struct GnuHashSection {
    pub(crate) view: Box<dyn SymbolsView>,
    pub(crate) symbols: SectionId,
}

impl GnuHashSection {
    pub(crate) fn new(view: impl SymbolsView + 'static, symbols: SectionId) -> Self {
        Self { view: Box::new(view), symbols }
    }
}

#[derive(Debug, Getters)]
pub(crate) struct RelocationsSection {
    #[get]
    section: SectionId,
    #[get]
    symbols_table: SectionId,
    relocations: Vec<Relocation>,
}

impl RelocationsSection {
    pub(crate) fn new(
        section: SectionId,
        symbols_table: SectionId,
        relocations: Vec<Relocation>,
    ) -> Self {
        Self { section, symbols_table, relocations }
    }

    pub(crate) fn relocations(&self) -> &[Relocation] {
        &self.relocations
    }

    pub(crate) fn relocations_mut(&mut self) -> &mut [Relocation] {
        &mut self.relocations
    }
}

#[derive(Debug, Getters)]
pub(crate) struct DynamicSection {
    #[get]
    strings: SectionId,
}

impl DynamicSection {
    pub(crate) fn new(strings: SectionId) -> Self {
        Self { strings }
    }
}

#[derive(Debug)]
pub(crate) struct NotesSection {
    pub(crate) notes: Vec<ElfNote>,
}

impl NotesSection {
    pub(crate) fn new(notes: Vec<ElfNote>) -> Self {
        Self { notes }
    }

    pub(crate) fn alignment(&self, class: ElfClass) -> u64 {
        let mut align = 1;
        for note in &self.notes {
            align = align.max(match note {
                // GNU properties is the only kind of note requiring an alignment of 8 on x86_64.
                ElfNote::GnuProperties(_) => match class {
                    ElfClass::Elf32 => 4,
                    ElfClass::Elf64 => 8,
                },
                _ => 4,
            });
        }
        align
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
from!(impl From<StringsSection> for SectionContent::Strings);
from!(impl From<SymbolsSection> for SectionContent::Symbols);
from!(impl From<SysvHashSection> for SectionContent::SysvHash);
from!(impl From<GnuHashSection> for SectionContent::GnuHash);
from!(impl From<RelocationsSection> for SectionContent::Relocations);
from!(impl From<DynamicSection> for SectionContent::Dynamic);
from!(impl From<NotesSection> for SectionContent::Notes);

#[derive(Debug)]
struct RemovedSection {
    name: Interned<String>,
}
