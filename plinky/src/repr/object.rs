use crate::cli::Mode;
use crate::passes::generate_got::GOT;
use crate::repr::sections::Sections;
use crate::repr::segments::Segments;
use crate::repr::symbols::Symbols;
use plinky_elf::ids::serial::{SectionId, SymbolId};
use plinky_elf::ElfEnvironment;

#[derive(Debug)]
pub(crate) struct Object {
    pub(crate) env: ElfEnvironment,
    pub(crate) sections: Sections,
    pub(crate) segments: Segments,
    pub(crate) symbols: Symbols,
    pub(crate) dynamic_entries: Vec<DynamicEntry>,
    pub(crate) got: Option<GOT>,
    pub(crate) entry_point: SymbolId,
    pub(crate) mode: Mode,
    pub(crate) executable_stack: bool,
    pub(crate) gnu_stack_section_ignored: bool,
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
