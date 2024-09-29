use plinky_elf::ids::serial::SectionId;
use plinky_utils::bitfields::Bitfield;
use plinky_elf::ElfDynamicFlags1;

#[derive(Debug)]
pub(crate) struct DynamicEntries {
    entries: Vec<DynamicEntry>,
    pub(crate) flags1: ElfDynamicFlags1,
}

impl DynamicEntries {
    pub(crate) fn new() -> Self {
        Self { entries: Vec::new(), flags1: ElfDynamicFlags1::empty() }
    }

    pub(crate) fn add(&mut self, entry: DynamicEntry) {
        self.entries.push(entry);
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = &DynamicEntry> {
        self.entries
            .iter()
            .chain(Some(&DynamicEntry::Flags1).filter(|_| !self.flags1.is_empty()).into_iter())
    }
}

#[derive(Debug)]
pub(crate) enum DynamicEntry {
    StringTable(SectionId),
    SymbolTable(SectionId),
    Hash(SectionId),
    GotRela(SectionId),
    Plt { got_plt: SectionId, rela: SectionId },
    Flags1,
}

impl DynamicEntry {
    pub(crate) fn directives_count(&self) -> usize {
        match self {
            DynamicEntry::StringTable(_) => 2,
            DynamicEntry::SymbolTable(_) => 2,
            DynamicEntry::Hash(_) => 1,
            DynamicEntry::GotRela(_) => 3,
            DynamicEntry::Plt { .. } => 4,
            DynamicEntry::Flags1 => 1,
        }
    }
}
