use crate::interner::Interned;
use crate::repr::strings::Strings;
use crate::repr::symbols::Symbols;
use plinky_diagnostics::ObjectSpan;
use plinky_elf::ids::serial::{SectionId, SerialIds};
use plinky_elf::{ElfDeduplication, ElfEnvironment, ElfPermissions, ElfRelocation, RawBytes};
use std::collections::BTreeMap;

#[derive(Debug)]
pub(crate) struct Object {
    pub(crate) env: ElfEnvironment,
    pub(crate) sections: BTreeMap<Interned<String>, Section>,
    pub(crate) section_ids_to_names: BTreeMap<SectionId, Interned<String>>,
    pub(crate) strings: Strings,
    pub(crate) symbols: Symbols,
}

#[derive(Debug)]
pub(crate) struct Section {
    pub(crate) perms: ElfPermissions,
    pub(crate) content: SectionContent,
}

#[derive(Debug)]
pub(crate) enum SectionContent {
    Data(DataSection),
    Uninitialized(BTreeMap<SectionId, UninitializedSectionPart>),
}

#[derive(Debug)]
pub(crate) struct DataSection {
    pub(crate) deduplication: ElfDeduplication,
    pub(crate) parts: BTreeMap<SectionId, DataSectionPart>,
}

#[derive(Debug)]
pub(crate) struct DataSectionPart {
    pub(crate) source: ObjectSpan,
    pub(crate) bytes: RawBytes,
    pub(crate) relocations: Vec<ElfRelocation<SerialIds>>,
}

#[derive(Debug)]
pub(crate) struct UninitializedSectionPart {
    pub(crate) source: ObjectSpan,
    pub(crate) len: u64,
}
