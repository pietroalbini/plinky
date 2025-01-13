use crate::repr::sections::SectionId;
use plinky_elf::writer::layout::Layout;
use plinky_macros::{Display, Error};
use plinky_utils::ints::{Address, Offset, OutOfBoundsError};

pub(crate) struct AddressResolver<'a> {
    layout: &'a Layout<SectionId>,
}

impl<'a> AddressResolver<'a> {
    pub(crate) fn new(layout: &'a Layout<SectionId>) -> Self {
        Self { layout }
    }

    pub(crate) fn address(
        &self,
        section: SectionId,
        offset: Offset,
    ) -> Result<(SectionId, Address), AddressResolutionError> {
        match &self.layout.metadata_of_section(&section).memory {
            Some(mem) => Ok((section, mem.address.offset(offset)?)),
            None => Err(AddressResolutionError::PointsToUnallocatedSection(section)),
        }
    }
}

#[derive(Debug, Display, Error)]
pub(crate) enum AddressResolutionError {
    #[display("address points to section {f0:?}, which is not going to be allocated in memory")]
    PointsToUnallocatedSection(SectionId),
    #[transparent]
    OutOfBounds(OutOfBoundsError),
}
