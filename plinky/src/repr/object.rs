use crate::interner::Interned;
use crate::passes::layout::Layout;
use crate::repr::strings::Strings;
use crate::repr::symbols::{MissingGlobalSymbol, SymbolValue, Symbols};
use plinky_diagnostics::ObjectSpan;
use plinky_elf::ids::serial::{SectionId, SerialIds};
use plinky_elf::{ElfDeduplication, ElfEnvironment, ElfPermissions, ElfRelocation, RawBytes};
use plinky_macros::{Display, Error};
use std::collections::BTreeMap;

#[derive(Debug)]
pub(crate) struct Object {
    pub(crate) env: ElfEnvironment,
    pub(crate) sections: BTreeMap<Interned<String>, Section>,
    pub(crate) section_ids_to_names: BTreeMap<SectionId, Interned<String>>,
    pub(crate) strings: Strings,
    pub(crate) symbols: Symbols,
}

impl Object {
    pub(crate) fn global_symbol_address(
        &self,
        layout: &Layout,
        name: Interned<String>,
    ) -> Result<u64, GetSymbolAddressError> {
        let symbol = self.symbols.get_global(name)?;

        // TODO: find a way to deduplicate this with the copy of the function in the relocation.
        match symbol.value {
            SymbolValue::Undefined => Err(GetSymbolAddressError::Undefined(name.into())),
            SymbolValue::Absolute { .. } => Err(GetSymbolAddressError::NotAnAddress(name.into())),
            SymbolValue::SectionRelative { section, offset } => Ok(layout.of(section) + offset),
        }
    }
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
pub(crate) enum DataSectionPart {
    Real(DataSectionPartReal),
    DeduplicationFacade(DeduplicationFacade),
}

#[derive(Debug)]
pub(crate) struct DataSectionPartReal {
    pub(crate) source: ObjectSpan,
    pub(crate) bytes: RawBytes,
    pub(crate) relocations: Vec<ElfRelocation<SerialIds>>,
}

#[derive(Debug, Clone)]
pub(crate) struct DeduplicationFacade {
    pub(crate) section_id: SectionId,
    pub(crate) source: ObjectSpan,
    pub(crate) offset_map: BTreeMap<u64, u64>,
}

#[derive(Debug)]
pub(crate) struct UninitializedSectionPart {
    pub(crate) source: ObjectSpan,
    pub(crate) len: u64,
}

#[derive(Debug, Error, Display)]
pub(crate) enum GetSymbolAddressError {
    #[transparent]
    Missing(MissingGlobalSymbol),
    #[display("symbol {f0} is undefined")]
    Undefined(Interned<String>),
    #[display("symbol {f0} is not an address")]
    NotAnAddress(Interned<String>),
}
