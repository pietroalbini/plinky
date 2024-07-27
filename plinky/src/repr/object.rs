use crate::cli::Mode;
use crate::passes::generate_got::GOT;
use crate::repr::relocations::Relocation;
use crate::repr::sections::Sections;
use crate::repr::segments::Segment;
use crate::repr::symbols::Symbols;
use plinky_elf::ids::serial::{SectionId, SymbolId};
use plinky_elf::ElfEnvironment;

#[derive(Debug)]
pub(crate) struct Object {
    pub(crate) env: ElfEnvironment,
    pub(crate) sections: Sections,
    pub(crate) segments: Vec<Segment>,
    pub(crate) symbols: Symbols,
    pub(crate) dynamic_relocations: Vec<Relocation>,
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
}
