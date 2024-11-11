use crate::repr::sections::{SectionId, UpcomingStringId};
use plinky_elf::{ElfDynamicFlags, ElfDynamicFlags1};
use plinky_utils::bitfields::Bitfield;

#[derive(Debug)]
pub(crate) struct DynamicEntries {
    entries: Vec<DynamicEntry>,
    pub(crate) flags: ElfDynamicFlags,
    pub(crate) flags1: ElfDynamicFlags1,
}

impl DynamicEntries {
    pub(crate) fn new() -> Self {
        Self {
            entries: Vec::new(),
            flags: ElfDynamicFlags::empty(),
            flags1: ElfDynamicFlags1::empty(),
        }
    }

    pub(crate) fn add(&mut self, entry: DynamicEntry) {
        self.entries.push(entry);
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = &DynamicEntry> {
        self.entries
            .iter()
            .chain(Some(&DynamicEntry::Flags).filter(|_| !self.flags.is_empty()).into_iter())
            .chain(Some(&DynamicEntry::Flags1).filter(|_| !self.flags1.is_empty()).into_iter())
    }
}

#[derive(Debug)]
pub(crate) enum DynamicEntry {
    SharedObjectName(UpcomingStringId),
    StringTable(SectionId),
    SymbolTable(SectionId),
    Hash(SectionId),
    GotReloc(SectionId),
    Plt { got_plt: SectionId, reloc: SectionId },
    Flags,
    Flags1,
}

impl DynamicEntry {
    pub(crate) fn directives_count(&self) -> usize {
        match self {
            DynamicEntry::SharedObjectName(_) => 1,
            DynamicEntry::StringTable(_) => 2,
            DynamicEntry::SymbolTable(_) => 2,
            DynamicEntry::Hash(_) => 1,
            DynamicEntry::GotReloc(_) => 3,
            DynamicEntry::Plt { .. } => 4,
            DynamicEntry::Flags => 1,
            DynamicEntry::Flags1 => 1,
        }
    }
}
