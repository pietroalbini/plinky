use crate::interner::Interned;
use crate::repr::object::{Object, Section, SectionContent, SectionLayout, SectionMerge};
use plink_elf::ids::serial::SectionId;
use plink_elf::ElfPermissions;
use plink_macros::{Display, Error};
use std::collections::BTreeMap;

const BASE_ADDRESS: u64 = 0x400000;
const PAGE_SIZE: u64 = 0x1000;

pub(crate) fn run(
    object: Object<()>,
) -> Result<(Object<SectionLayout>, Vec<SectionMerge>), LayoutCalculatorError> {
    let mut calculator = LayoutCalculator::new();
    for (id, section) in &object.sections {
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
        env: object.env,
        sections: object
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
        strings: object.strings,
        symbols: object.symbols,
    };

    Ok((object, layout.merges))
}

struct LayoutCalculator {
    sections: BTreeMap<Interned<String>, Vec<SectionToLayout>>,
}

impl<'a> LayoutCalculator {
    fn new() -> Self {
        Self {
            sections: BTreeMap::new(),
        }
    }

    fn learn_section(
        &mut self,
        id: SectionId,
        name: Interned<String>,
        len: usize,
        perms: ElfPermissions,
    ) -> Result<(), LayoutCalculatorError> {
        self.sections
            .entry(name)
            .or_default()
            .push(SectionToLayout { id, len, perms });
        Ok(())
    }

    fn calculate(self) -> Result<CalculatedLayout, LayoutCalculatorError> {
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

struct CalculatedLayout {
    sections: BTreeMap<SectionId, SectionLayout>,
    merges: Vec<SectionMerge>,
}

#[derive(Debug, Error, Display)]
pub(crate) enum LayoutCalculatorError {
    #[display("instances of section {f0} have different perms: {f1:?} vs {f2:?}")]
    SectionWithDifferentPerms(Interned<String>, ElfPermissions, ElfPermissions),
}
