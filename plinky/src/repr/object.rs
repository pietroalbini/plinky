use crate::cli::Mode;
use crate::passes::generate_got::GOT;
use crate::passes::generate_plt::Plt;
use crate::repr::dynamic_entries::DynamicEntries;
use crate::repr::sections::Sections;
use crate::repr::segments::Segments;
use crate::repr::symbols::{SymbolId, Symbols};
use plinky_diagnostics::ObjectSpan;
use plinky_elf::{ElfEnvironment, ElfX86Features2, ElfX86Isa};

#[derive(Debug)]
pub(crate) struct Object {
    pub(crate) env: ElfEnvironment,
    pub(crate) inputs: Vec<Input>,
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

#[derive(Debug)]
pub(crate) struct Input {
    pub(crate) span: ObjectSpan,
    pub(crate) x86_isa_used: Option<ElfX86Isa>,
    pub(crate) x86_features_2_used: Option<ElfX86Features2>,
}
