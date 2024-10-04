use crate::passes::deduplicate::Deduplication;
use crate::repr::sections::SectionId;
use plinky_elf::writer::layout::Layout;
use plinky_macros::{Display, Error};
use plinky_utils::ints::{Address, Offset, OutOfBoundsError};
use std::collections::BTreeMap;

pub(crate) struct AddressResolver<'a> {
    layout: &'a Layout<SectionId>,
    deduplications: &'a BTreeMap<SectionId, Deduplication>,
}

impl<'a> AddressResolver<'a> {
    pub(crate) fn new(
        layout: &'a Layout<SectionId>,
        deduplications: &'a BTreeMap<SectionId, Deduplication>,
    ) -> Self {
        Self { layout, deduplications }
    }

    pub(crate) fn address(
        &self,
        section: SectionId,
        offset: Offset,
    ) -> Result<(SectionId, Address), AddressResolutionError> {
        if let Some(deduplication) = self.deduplications.get(&section) {
            let base = match &self.layout.metadata_of_section(&deduplication.target).memory {
                Some(mem) => mem.address,
                None => {
                    return Err(AddressResolutionError::PointsToUnallocatedSection(
                        deduplication.target,
                    ))
                }
            };

            match deduplication.map.get(&offset) {
                Some(&mapped) => Ok((deduplication.target, base.offset(mapped)?)),
                None => Err(AddressResolutionError::UnalignedReferenceToDeduplication),
            }
        } else {
            match &self.layout.metadata_of_section(&section).memory {
                Some(mem) => Ok((section, mem.address.offset(offset)?)),
                None => Err(AddressResolutionError::PointsToUnallocatedSection(section)),
            }
        }
    }
}

#[derive(Debug, Display, Error)]
pub(crate) enum AddressResolutionError {
    #[display("address points to section {f0:?}, which is not going to be allocated in memory")]
    PointsToUnallocatedSection(SectionId),
    #[display("referenced an offset not aligned to the deduplication boundaries")]
    UnalignedReferenceToDeduplication,
    #[transparent]
    OutOfBounds(OutOfBoundsError),
}
