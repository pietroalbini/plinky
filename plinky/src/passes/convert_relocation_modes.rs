//! Different architectures expect a different relocation mode in the output objects. This module
//! takes care of converting whatever mode is present in the relocations into the expected one.

use crate::repr::object::Object;
use crate::repr::relocations::{RelocationAddend, RelocationMode};
use crate::repr::sections::SectionContent;
use plinky_elf::ElfEndian;
use plinky_macros::{Display, Error};
use plinky_utils::ints::{ExtractNumber, Offset};
use std::collections::BTreeMap;

const ADDEND_BYTES: usize = 4;

pub(crate) fn run(object: &mut Object) -> Result<(), ConvertRelocationModesError> {
    let endian = object.env.endian;
    let expected_mode = object.relocation_mode();

    // To avoid borrow checker problems, we categorize the sections ahead of time.
    let mut targets = BTreeMap::new();
    let mut to_process = Vec::new();
    for section in object.sections.iter_mut() {
        match &mut section.content {
            SectionContent::Relocations(relocations) => to_process.push(relocations),
            SectionContent::Data(data) => {
                targets.insert(section.id, data);
            }
            _ => {}
        }
    }

    for relocations in to_process {
        let applies_to = targets
            .get_mut(&relocations.section())
            .expect("relocations does not point to a data section");

        for relocation in relocations.relocations_mut() {
            match (&relocation.addend, expected_mode) {
                (RelocationAddend::Inline, RelocationMode::Rela) => {
                    if relocation.type_.uses_addend() {
                        let addend_bytes = *addend_slice(&mut applies_to.bytes, relocation.offset);
                        relocation.addend = RelocationAddend::Explicit(match endian {
                            ElfEndian::Little => i32::from_le_bytes(addend_bytes).into(),
                        });
                    } else {
                        relocation.addend = RelocationAddend::Explicit(0.into());
                    }
                }

                (RelocationAddend::Explicit(addend), RelocationMode::Rel) => {
                    if relocation.type_.uses_addend() {
                        let addend: i32 = addend.extract().try_into().map_err(|_| {
                            ConvertRelocationModesError::RelAddendMustBe32Bit(addend.extract())
                        })?;
                        *addend_slice(&mut applies_to.bytes, relocation.offset) = match endian {
                            ElfEndian::Little => addend.to_le_bytes(),
                        };
                    }
                    relocation.addend = RelocationAddend::Inline;
                }

                // Already of the correct mode:
                (RelocationAddend::Explicit(_), RelocationMode::Rela) => {}
                (RelocationAddend::Inline, RelocationMode::Rel) => {}
            }
        }
    }

    Ok(())
}

fn addend_slice(bytes: &mut [u8], offset: Offset) -> &mut [u8; 4] {
    let start = offset.extract();
    let end = offset.extract() + ADDEND_BYTES as i64;
    if start < 0 || (bytes.len() as i64) < end {
        panic!("relocation's addend is out of bounds");
    }

    (&mut bytes[(start as usize)..(end as usize)]).try_into().unwrap()
}

#[derive(Debug, Error, Display)]
pub(crate) enum ConvertRelocationModesError {
    #[display("relocation addend {f0} does not fit in 32 bits")]
    RelAddendMustBe32Bit(i64),
}
