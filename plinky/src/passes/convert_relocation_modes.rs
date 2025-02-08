//! Different architectures expect a different relocation mode in the output objects. This module
//! takes care of converting whatever mode is present in the relocations into the expected one.

use crate::repr::object::Object;
use crate::repr::relocations::{
    AddendType, RelocationAddend, RelocationAddendError, RelocationMode,
};
use crate::repr::sections::SectionContent;
use plinky_elf::ElfEndian;
use plinky_macros::{Display, Error};
use plinky_utils::ints::{ExtractNumber, Offset};
use std::collections::BTreeMap;

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
                    let addend = match relocation.addend(endian, &applies_to.bytes) {
                        Ok(addend) => addend,
                        Err(RelocationAddendError::NotSupported(_)) => Offset::from(0i64),
                        Err(RelocationAddendError::OutOfBounds(offset)) => {
                            return Err(ConvertRelocationModesError::OutOfBounds(offset));
                        }
                    };
                    relocation.addend = RelocationAddend::Explicit(addend);
                }

                (RelocationAddend::Explicit(addend), RelocationMode::Rel) => {
                    match relocation.type_.addend_type() {
                        AddendType::None => {}
                        AddendType::I32 => {
                            let addend: i32 = addend.extract().try_into().map_err(|_| {
                                ConvertRelocationModesError::RelAddendMustBe32Bit(*addend)
                            })?;
                            write_addend(&mut applies_to.bytes, relocation.offset, match endian {
                                ElfEndian::Little => addend.to_le_bytes(),
                            })?;
                        }
                        AddendType::I64 => {
                            write_addend(&mut applies_to.bytes, relocation.offset, match endian {
                                ElfEndian::Little => addend.extract().to_le_bytes(),
                            })?;
                        }
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

fn write_addend<const N: usize>(
    bytes: &mut [u8],
    offset: Offset,
    addend: [u8; N],
) -> Result<(), ConvertRelocationModesError> {
    let start = offset.extract();
    let end = offset.extract() + N as i64;
    if start < 0 || (bytes.len() as i64) < end {
        return Err(ConvertRelocationModesError::OutOfBounds(offset));
    }

    let slot = &mut bytes[(start as usize)..(end as usize)].try_into().unwrap();
    *slot = addend;

    Ok(())
}

#[derive(Debug, Error, Display)]
pub(crate) enum ConvertRelocationModesError {
    #[display("relocation addend {f0} does not fit in 32 bits")]
    RelAddendMustBe32Bit(Offset),
    #[display("addend at offset {f0:?} is out of section bounds")]
    OutOfBounds(Offset),
}
