use crate::linker::strings::{MissingStringError, Strings};
use plink_elf::ids::serial::{SectionId, StringId};
use std::collections::BTreeMap;

const BASE_ADDRESS: u64 = 0x400000;
const PAGE_SIZE: u64 = 0x1000;

pub(super) struct LayoutCalculator<'a> {
    sections: BTreeMap<String, Vec<SectionToLayout>>,
    strings: &'a Strings,
}

impl<'a> LayoutCalculator<'a> {
    pub(super) fn new(strings: &'a Strings) -> Self {
        Self {
            sections: BTreeMap::new(),
            strings,
        }
    }

    pub(super) fn learn_section(
        &mut self,
        id: SectionId,
        name: StringId,
        len: usize,
    ) -> Result<(), LayoutCalculatorError> {
        let name = self
            .strings
            .get(name)
            .map_err(|e| LayoutCalculatorError::MissingSectionName(id, e))?;
        self.sections
            .entry(name.into())
            .or_insert_with(Vec::new)
            .push(SectionToLayout { id, len });
        Ok(())
    }

    pub(super) fn calculate(self) -> CalculatedLayout {
        let mut calculated = CalculatedLayout {
            sections: BTreeMap::new(),
            merges: Vec::new(),
        };

        let mut address = BASE_ADDRESS;
        for (name, sections) in self.sections {
            let mut merge = SectionMerge {
                name,
                address,
                sections: Vec::new(),
            };
            for section in sections {
                calculated
                    .sections
                    .insert(section.id, SectionLayout { address });
                merge.sections.push(section.id);
                address += section.len as u64;
            }
            calculated.merges.push(merge);

            // Align to the next page boundary.
            address = (address + PAGE_SIZE) & !(PAGE_SIZE - 1);
        }
        calculated
    }
}

struct SectionToLayout {
    id: SectionId,
    len: usize,
}

pub(super) struct CalculatedLayout {
    pub(super) sections: BTreeMap<SectionId, SectionLayout>,
    pub(super) merges: Vec<SectionMerge>,
}

pub(crate) struct SectionLayout {
    pub(super) address: u64,
}

#[derive(Debug)]
pub(super) struct SectionMerge {
    name: String,
    address: u64,
    sections: Vec<SectionId>,
}

#[derive(Debug)]
pub(crate) enum LayoutCalculatorError {
    MissingSectionName(SectionId, MissingStringError),
}

impl std::error::Error for LayoutCalculatorError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            LayoutCalculatorError::MissingSectionName(_, err) => Some(err),
        }
    }
}

impl std::fmt::Display for LayoutCalculatorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LayoutCalculatorError::MissingSectionName(id, _) => {
                write!(f, "failed to read name for section {id:?}")
            }
        }
    }
}
