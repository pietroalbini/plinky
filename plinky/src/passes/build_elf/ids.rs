use plinky_elf::new_serial_ids;

new_serial_ids! {
    pub(crate) BuiltElfIds {
        type SectionId = pub(crate) BuiltElfSectionId;
        type SymbolId = pub(crate) BuiltElfSymbolId;
        type StringId = pub(crate) BuiltElfStringId;
    }
}
