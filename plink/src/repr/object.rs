use crate::repr::layout::{LayoutCalculator, LayoutCalculatorError, SectionLayout, SectionMerge};
use crate::repr::relocator::{RelocationError, Relocator};
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

impl Object<()> {
    pub(crate) fn calculate_layout(
        self,
    ) -> Result<(Object<SectionLayout>, Vec<SectionMerge>), LayoutCalculatorError> {
        let mut calculator = LayoutCalculator::new(&self.strings);
        for (id, section) in &self.sections {
            calculator.learn_section(
                *id,
                section.name,
                match &section.content {
                    SectionContent::Data(data) => data.bytes.len(),
                    SectionContent::Uninitialized(uninit) => uninit.len as usize,
                },
                section.perms,
            )?;
        }

        let mut layout = calculator.calculate()?;
        let object = Object {
            env: self.env,
            sections: self
                .sections
                .into_iter()
                .map(|(id, section)| {
                    (
                        id,
                        Section {
                            name: section.name,
                            perms: section.perms,
                            content: section.content,
                            layout: layout.sections.remove(&id).unwrap(),
                        },
                    )
                })
                .collect(),
            strings: self.strings,
            symbols: self.symbols,
        };

        Ok((object, layout.merges))
    }
}

impl Object<SectionLayout> {
    pub(crate) fn relocate(&mut self) -> Result<(), RelocationError> {
        let relocator = Relocator::new(self.section_layouts(), &self.symbols);
        for (id, section) in &mut self.sections.iter_mut() {
            match &mut section.content {
                SectionContent::Data(data) => relocator.relocate(*id, data)?,
                SectionContent::Uninitialized(_) => {}
            }
        }
        Ok(())
    }

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

    pub(crate) fn section_layouts(&self) -> impl Iterator<Item = (SectionId, &'_ SectionLayout)> {
        self.sections
            .iter()
            .map(|(id, section)| (*id, &section.layout))
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

#[derive(Debug, Error, Display)]
pub(crate) enum GetSymbolAddressError {
    #[display("could not find the symbol")]
    Missing(#[from] MissingGlobalSymbol),
    #[display("symbol {f0} is undefined")]
    Undefined(String),
    #[display("symbol {f0} is not an address")]
    NotAnAddress(String),
}
