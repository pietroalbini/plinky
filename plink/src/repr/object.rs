use crate::interner::Interned;
use crate::repr::strings::Strings;
use crate::repr::symbols::{MissingGlobalSymbol, Symbols};
use plink_elf::ids::serial::{SectionId, SerialIds};
use plink_elf::{
    ElfDeduplication, ElfEnvironment, ElfPermissions, ElfRelocation, ElfSymbolDefinition, RawBytes,
};
use plink_macros::{Display, Error};
use std::collections::BTreeMap;

#[derive(Debug)]
pub(crate) struct Object<L> {
    pub(crate) env: ElfEnvironment,
    pub(crate) sections: BTreeMap<Interned<String>, Section<L>>,
    pub(crate) section_ids_to_names: BTreeMap<SectionId, Interned<String>>,
    pub(crate) strings: Strings,
    pub(crate) symbols: Symbols,
}

impl Object<SectionLayout> {
    pub(crate) fn global_symbol_address(&self, name: &str) -> Result<u64, GetSymbolAddressError> {
        let symbol = self.symbols.get_global(name)?;

        match symbol.definition {
            ElfSymbolDefinition::Undefined => Err(GetSymbolAddressError::Undefined(name.into())),
            ElfSymbolDefinition::Absolute => Err(GetSymbolAddressError::NotAnAddress(name.into())),
            ElfSymbolDefinition::Common => todo!(),
            ElfSymbolDefinition::Section(section_id) => {
                let section_addr = self
                    .section_ids_to_names
                    .get(&section_id)
                    .and_then(|name| self.sections.get(name))
                    .and_then(|section| match &section.content {
                        SectionContent::Data(data) => {
                            data.parts.get(&section_id).map(|p| p.layout.address)
                        }
                        SectionContent::Uninitialized(uninit) => {
                            uninit.get(&section_id).map(|p| p.layout.address)
                        }
                    })
                    .expect("invalid section id");
                Ok(section_addr + symbol.value)
            }
        }
    }
}

#[derive(Debug)]
pub(crate) struct Section<L> {
    pub(crate) perms: ElfPermissions,
    pub(crate) content: SectionContent<L>,
}

#[derive(Debug)]
pub(crate) enum SectionContent<L> {
    Data(DataSection<L>),
    Uninitialized(BTreeMap<SectionId, UninitializedSectionPart<L>>),
}

#[derive(Debug)]
pub(crate) struct DataSection<L> {
    pub(crate) deduplication: ElfDeduplication,
    pub(crate) parts: BTreeMap<SectionId, DataSectionPart<L>>,
}

#[derive(Debug)]
pub(crate) struct DataSectionPart<L> {
    pub(crate) bytes: RawBytes,
    pub(crate) relocations: Vec<ElfRelocation<SerialIds>>,
    pub(crate) layout: L,
}

#[derive(Debug)]
pub(crate) struct UninitializedSectionPart<L> {
    pub(crate) len: u64,
    pub(crate) layout: L,
}

#[derive(Debug)]
pub(crate) struct SectionLayout {
    pub(crate) address: u64,
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
