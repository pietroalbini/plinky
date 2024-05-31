use crate::passes::generate_got::GOT;
use crate::repr::sections::Sections;
use crate::repr::symbols::Symbols;
use plinky_elf::ids::serial::SymbolId;
use plinky_elf::ElfEnvironment;

#[derive(Debug)]
pub(crate) struct Object {
    pub(crate) env: ElfEnvironment,
    pub(crate) sections: Sections,
    pub(crate) symbols: Symbols,
    pub(crate) got: Option<GOT>,
    pub(crate) entry_point: SymbolId,
    pub(crate) executable_stack: bool,
    pub(crate) gnu_stack_section_ignored: bool,
}
