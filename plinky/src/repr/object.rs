use crate::cli::Mode;
use crate::passes::generate_got::GOT;
use crate::passes::generate_plt::Plt;
use crate::repr::dynamic_entries::DynamicEntries;
use crate::repr::sections::Sections;
use crate::repr::segments::Segments;
use crate::repr::symbols::{SymbolId, Symbols};
use plinky_elf::ElfEnvironment;

#[derive(Debug)]
pub(crate) struct Object {
    pub(crate) env: ElfEnvironment,
    pub(crate) sections: Sections,
    pub(crate) segments: Segments,
    pub(crate) symbols: Symbols,
    pub(crate) dynamic_entries: DynamicEntries,
    pub(crate) got: Option<GOT>,
    pub(crate) got_plt: Option<GOT>,
    pub(crate) plt: Option<Plt>,
    pub(crate) entry_point: Option<SymbolId>,
    pub(crate) mode: Mode,
    pub(crate) executable_stack: bool,
    pub(crate) gnu_stack_section_ignored: bool,
}
