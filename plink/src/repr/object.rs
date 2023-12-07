use crate::repr::strings::Strings;
use crate::repr::symbols::{MissingGlobalSymbol, Symbols};
use plink_elf::ids::serial::{SectionId, SerialIds, StringId};
use plink_elf::{ElfEnvironment, ElfPermissions, ElfRelocation, ElfSymbolDefinition, RawBytes};
use plink_macros::{Display, Error};
use std::collections::BTreeMap;

#[derive(Debug)]
pub(crate) struct Object<L> {
    pub(crate) env: ElfEnvironment,
    pub(crate) sections: BTreeMap<SectionId, Section<L>>,
    pub(crate) strings: Strings,
    pub(crate) symbols: Symbols,
}

impl Object<SectionLayout> {
    pub(crate) fn take_section(&mut self, id: SectionId) -> Section<SectionLayout> {
        self.sections.remove(&id).expect("invalid section id")
    }

    pub(crate) fn global_symbol_address(&self, name: &str) -> Result<u64, GetSymbolAddressError> {
        let symbol = self.symbols.get_global(name)?;

        match symbol.definition {
            ElfSymbolDefinition::Undefined => Err(GetSymbolAddressError::Undefined(name.into())),
            ElfSymbolDefinition::Absolute => Err(GetSymbolAddressError::NotAnAddress(name.into())),
            ElfSymbolDefinition::Common => todo!(),
            ElfSymbolDefinition::Section(section_id) => {
                let section_offset = self
                    .sections
                    .get(&section_id)
                    .expect("invalid section id")
                    .layout
                    .address;
                Ok(section_offset + symbol.value)
            }
        }
    }
}

#[derive(Debug)]
pub(crate) struct Section<L> {
    pub(crate) name: StringId,
    pub(crate) perms: ElfPermissions,
    pub(crate) content: SectionContent,
    pub(crate) layout: L,
}

#[derive(Debug)]
pub(crate) enum SectionContent {
    Data(DataSection),
    Uninitialized(UninitializedSection),
}

#[derive(Debug)]
pub(crate) struct DataSection {
    pub(crate) bytes: RawBytes,
    pub(crate) relocations: Vec<ElfRelocation<SerialIds>>,
}

#[derive(Debug)]
pub(crate) struct UninitializedSection {
    pub(crate) len: u64,
}

#[derive(Debug)]
pub(crate) struct SectionLayout {
    pub(crate) address: u64,
}

#[derive(Debug)]
pub(crate) struct SectionMerge {
    pub(crate) name: String,
    pub(crate) address: u64,
    pub(crate) perms: ElfPermissions,
    pub(crate) sections: Vec<SectionId>,
}

#[derive(Debug, Error, Display)]
pub(crate) enum GetSymbolAddressError {
    #[display("could not find the symbol")]
    Missing(#[from] MissingGlobalSymbol),
    #[display("symbol {f0} is undefined")]
    Undefined(String),
    #[display("symbol {f0} is not an address")]
    NotAnAddress(String),
}
