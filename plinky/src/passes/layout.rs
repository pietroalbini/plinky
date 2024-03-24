use crate::passes::deduplicate::Deduplication;
use crate::repr::object::{Object, SectionContent};
use plinky_elf::ids::serial::SectionId;
use plinky_macros::{Display, Error};
use std::collections::BTreeMap;

const BASE_ADDRESS: u64 = 0x400000;
const PAGE_SIZE: u64 = 0x1000;

pub(crate) fn run(object: &Object, deduplications: BTreeMap<SectionId, Deduplication>) -> Layout {
    let mut layout = Layout { sections: BTreeMap::new(), deduplications };
    let mut calculator = LayoutCalculator::new();
    for section in object.sections.values() {
        let mut section_calculator =
            if section.perms.read || section.perms.write || section.perms.execute {
                calculator.begin_section()
            } else {
                calculator.begin_unallocated_section()
            };
        match &section.content {
            SectionContent::Data(parts) => {
                for (&id, part) in &parts.parts {
                    layout
                        .sections
                        .insert(id, section_calculator.layout_of(part.bytes.0.len() as _));
                }
            }
            SectionContent::Uninitialized(parts) => {
                for (&id, part) in parts {
                    layout.sections.insert(id, section_calculator.layout_of(part.len));
                }
            }
        }
    }

    layout
}

pub(crate) struct Layout {
    sections: BTreeMap<SectionId, SectionLayout>,
    deduplications: BTreeMap<SectionId, Deduplication>,
}

impl Layout {
    pub(crate) fn of_section(&self, id: SectionId) -> &SectionLayout {
        match self.sections.get(&id) {
            Some(layout) => layout,
            None => panic!("section {id:?} doesn't have a layout"),
        }
    }

    pub(crate) fn address(
        &self,
        section: SectionId,
        offset: i64,
    ) -> Result<u64, AddressResolutionError> {
        if let Some(deduplication) = self.deduplications.get(&section) {
            let base = match self.of_section(deduplication.target) {
                SectionLayout::Allocated { address } => *address,
                SectionLayout::NotAllocated => {
                    return Err(AddressResolutionError::PointsToUnallocatedSection(
                        deduplication.target,
                    ))
                }
            };

            let map_key = u64::try_from(offset)
                .map_err(|_| AddressResolutionError::NegativeOffsetToAccessDeduplications)?;
            match deduplication.map.get(&map_key) {
                Some(&mapped) => Ok(base + mapped),
                None => Err(AddressResolutionError::UnalignedReferenceToDeduplication),
            }
        } else {
            match self.of_section(section) {
                SectionLayout::Allocated { address } => Ok((*address as i64 + offset) as u64),
                SectionLayout::NotAllocated => {
                    Err(AddressResolutionError::PointsToUnallocatedSection(section))
                }
            }
        }
    }

    pub(crate) fn iter_deduplications(&self) -> impl Iterator<Item = (SectionId, &Deduplication)> {
        self.deduplications.iter().map(|(id, dedup)| (*id, dedup))
    }
}

pub(crate) enum SectionLayout {
    Allocated { address: u64 },
    NotAllocated,
}

struct LayoutCalculator {
    address: u64,
}

impl LayoutCalculator {
    fn new() -> Self {
        Self { address: BASE_ADDRESS }
    }

    fn begin_section(&mut self) -> SectionLayoutCalculator<'_> {
        SectionLayoutCalculator { parent: self, allocate: true }
    }

    fn begin_unallocated_section(&mut self) -> SectionLayoutCalculator<'_> {
        SectionLayoutCalculator { parent: self, allocate: false }
    }
}

struct SectionLayoutCalculator<'a> {
    parent: &'a mut LayoutCalculator,
    allocate: bool,
}

impl SectionLayoutCalculator<'_> {
    fn layout_of(&mut self, len: u64) -> SectionLayout {
        if self.allocate {
            let layout = SectionLayout::Allocated { address: self.parent.address };
            self.parent.address += len;
            layout
        } else {
            SectionLayout::NotAllocated
        }
    }
}

impl Drop for SectionLayoutCalculator<'_> {
    fn drop(&mut self) {
        if self.allocate {
            // Align to the next page boundary when a section ends.
            self.parent.address = (self.parent.address + PAGE_SIZE) & !(PAGE_SIZE - 1);
        }
    }
}

#[derive(Debug, Display, Error)]
pub(crate) enum AddressResolutionError {
    #[display("address points to section {f0:?}, which is not going to be allocated in memory")]
    PointsToUnallocatedSection(SectionId),
    #[display("negative offset was used to access deduplications")]
    NegativeOffsetToAccessDeduplications,
    #[display("referenced an offset not aligned to the deduplication boundaries")]
    UnalignedReferenceToDeduplication,
}
