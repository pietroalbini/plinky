use crate::linker::strings::{MissingStringError, Strings};
use plink_elf::ids::serial::{SectionId, StringId};
use plink_elf::ElfPermissions;
use plink_macros::Error;
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
        perms: ElfPermissions,
    ) -> Result<(), LayoutCalculatorError> {
        let name = self
            .strings
            .get(name)
            .map_err(|e| LayoutCalculatorError::MissingSectionName(id, e))?;
        self.sections
            .entry(name.into())
            .or_default()
            .push(SectionToLayout { id, len, perms });
        Ok(())
    }

    pub(super) fn calculate(self) -> Result<CalculatedLayout, LayoutCalculatorError> {
        let mut calculated = CalculatedLayout {
            sections: BTreeMap::new(),
            merges: Vec::new(),
        };

        let mut address = BASE_ADDRESS;
        for (name, sections) in self.sections {
            let section_address = address;
            let mut section_ids = Vec::new();
            let mut perms = None;
            for section in sections {
                calculated
                    .sections
                    .insert(section.id, SectionLayout { address });
                section_ids.push(section.id);
                address += section.len as u64;

                match perms {
                    Some(existing) => {
                        if section.perms != existing {
                            return Err(LayoutCalculatorError::SectionWithDifferentPerms(
                                name,
                                existing,
                                section.perms,
                            ));
                        }
                    }
                    None => perms = Some(section.perms),
                }
            }
            calculated.merges.push(SectionMerge {
                name,
                address: section_address,
                perms: perms.unwrap(),
                sections: section_ids,
            });

            // Align to the next page boundary.
            address = (address + PAGE_SIZE) & !(PAGE_SIZE - 1);
        }
        Ok(calculated)
    }
}

struct SectionToLayout {
    id: SectionId,
    len: usize,
    perms: ElfPermissions,
}

pub(super) struct CalculatedLayout {
    pub(super) sections: BTreeMap<SectionId, SectionLayout>,
    pub(super) merges: Vec<SectionMerge>,
}

#[derive(Debug)]
pub(crate) struct SectionLayout {
    pub(crate) address: u64,
}

#[derive(Debug)]
pub(crate) struct SectionMerge {
    pub(super) name: String,
    pub(super) address: u64,
    pub(super) perms: ElfPermissions,
    pub(super) sections: Vec<SectionId>,
}

#[derive(Debug, Error)]
pub(crate) enum LayoutCalculatorError {
    MissingSectionName(SectionId, #[source] MissingStringError),
    SectionWithDifferentPerms(String, ElfPermissions, ElfPermissions),
}

impl std::fmt::Display for LayoutCalculatorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LayoutCalculatorError::MissingSectionName(id, _) => {
                write!(f, "failed to read name for section {id:?}")
            }
            LayoutCalculatorError::SectionWithDifferentPerms(name, perms1, perms2) => {
                write!(
                    f,
                    "instances of section {name} have different perms: {perms1:?} vs {perms2:?}"
                )
            }
        }
    }
}
