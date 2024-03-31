use crate::passes::deduplicate::Deduplication;
use crate::repr::object::{Object, SectionContent};
use plinky_elf::ids::serial::SectionId;
use plinky_elf::ElfPermissions;
use plinky_macros::{Display, Error};
use std::collections::BTreeMap;

const BASE_ADDRESS: u64 = 0x400000;
const PAGE_SIZE: u64 = 0x1000;

pub(crate) fn run(object: &Object, deduplications: BTreeMap<SectionId, Deduplication>) -> Layout {
    let mut grouped: BTreeMap<_, Vec<_>> = BTreeMap::new();
    for section in object.sections.iter() {
        match &section.content {
            SectionContent::Data(data) => grouped
                .entry((section.perms, SegmentType::Program))
                .or_default()
                .push((section.id, data.bytes.len() as u64)),
            SectionContent::Uninitialized(uninit) => grouped
                .entry((section.perms, SegmentType::Uninitialized))
                .or_default()
                .push((section.id, uninit.len)),
        }
    }

    let mut layout = Layout { segments: Vec::new(), sections: BTreeMap::new(), deduplications };
    let mut address = BASE_ADDRESS;
    for ((perms, type_), sections) in grouped.into_iter() {
        if perms.read || perms.write || perms.execute {
            let start = address;

            let mut segment_len = 0;
            for &(section, len) in &sections {
                layout.sections.insert(section, SectionLayout::Allocated { address });
                address += len;
                segment_len += len;
            }

            layout.segments.push(Segment {
                start,
                len: segment_len,
                perms,
                type_,
                align: PAGE_SIZE,
                sections: sections.iter().map(|(id, _)| *id).collect(),
            });

            // Align to the page boundary.
            address = (address + PAGE_SIZE) & !(PAGE_SIZE - 1);
        } else {
            // Avoid allocating sections that cannot be accessed at runtime.
            for (section, _) in sections {
                layout.sections.insert(section, SectionLayout::NotAllocated);
            }
        }
    }

    layout
}

pub(crate) struct Layout {
    segments: Vec<Segment>,
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

    pub(crate) fn iter_segments(&self) -> impl Iterator<Item = &Segment> {
        self.segments.iter()
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum SectionLayout {
    Allocated { address: u64 },
    NotAllocated,
}

pub(crate) struct Segment {
    pub(crate) start: u64,
    pub(crate) len: u64,
    pub(crate) align: u64,
    pub(crate) type_: SegmentType,
    pub(crate) perms: ElfPermissions,
    pub(crate) sections: Vec<SectionId>,
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum SegmentType {
    Program,
    Uninitialized,
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
