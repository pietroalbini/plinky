use plinky_elf::ids::serial::SectionId;

#[derive(Debug)]
pub(crate) struct DynamicEntries {
    entries: Vec<DynamicEntry>,
}

impl DynamicEntries {
    pub(crate) fn new() -> Self {
        Self { entries: Vec::new() }
    }

    pub(crate) fn add(&mut self, entry: DynamicEntry) {
        self.entries.push(entry);
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = &DynamicEntry> {
        self.entries.iter()
    }
}

#[derive(Debug)]
pub(crate) enum DynamicEntry {
    StringTable(SectionId),
    SymbolTable(SectionId),
    Hash(SectionId),
    Rela(SectionId),
    PieFlag,
}

impl DynamicEntry {
    pub(crate) fn directives_count(&self) -> usize {
        match self {
            DynamicEntry::StringTable(_) => 2,
            DynamicEntry::SymbolTable(_) => 2,
            DynamicEntry::Hash(_) => 1,
            DynamicEntry::Rela(_) => 3,
            DynamicEntry::PieFlag => 1,
        }
    }
}
