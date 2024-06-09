use crate::passes::deduplicate::Deduplication;
use crate::repr::object::Object;
use crate::repr::sections::SectionContent;
use crate::utils::ints::{Address, Offset, OutOfBoundsError};
use plinky_elf::ids::serial::SectionId;
use plinky_elf::ElfPermissions;
use plinky_macros::{Display, Error};
use std::collections::BTreeMap;

const BASE_ADDRESS: u64 = 0x400000;
const PAGE_SIZE: u64 = 0x1000;

pub(crate) fn run(
    object: &Object,
    deduplications: BTreeMap<SectionId, Deduplication>,
    interp_section: Option<SectionId>,
) -> Layout {
    let mut grouped: BTreeMap<_, Vec<_>> = BTreeMap::new();
    for section in object.sections.iter() {
        match &section.content {
            SectionContent::Data(data) => grouped
                .entry((
                    if Some(section.id) == interp_section {
                        SegmentType::Interpreter
                    } else {
                        SegmentType::Program
                    },
                    section.perms,
                ))
                .or_default()
                .push((section.id, data.bytes.len() as u64)),
            SectionContent::Uninitialized(uninit) => grouped
                .entry((SegmentType::Uninitialized, section.perms))
                .or_default()
                .push((section.id, uninit.len)),
        }
    }

    let mut layout = Layout {
        current_address: BASE_ADDRESS,
        segments: Vec::new(),
        sections: BTreeMap::new(),
        deduplications,
    };
    for ((type_, perms), sections) in grouped.into_iter() {
        if perms.read || perms.write || perms.execute {
            let mut segment = layout.prepare_segment();
            for &(section, len) in &sections {
                segment.add_section(section, len);
            }
            segment.finalize(type_, perms);
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
    current_address: u64,
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
        offset: Offset,
    ) -> Result<(SectionId, Address), AddressResolutionError> {
        if let Some(deduplication) = self.deduplications.get(&section) {
            let base = match self.of_section(deduplication.target) {
                SectionLayout::Allocated { address } => *address,
                SectionLayout::NotAllocated => {
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
            match self.of_section(section) {
                SectionLayout::Allocated { address } => Ok((section, address.offset(offset)?)),
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

    pub(crate) fn add_segment(&mut self, segment: Segment) {
        self.segments.push(segment);
    }

    pub(crate) fn prepare_segment(&mut self) -> PendingSegment {
        PendingSegment { start: self.current_address, sections: Vec::new(), len: 0, layout: self }
    }
}

pub(crate) struct PendingSegment<'a> {
    layout: &'a mut Layout,
    sections: Vec<SectionId>,
    start: u64,
    len: u64,
}

impl PendingSegment<'_> {
    pub(crate) fn add_section(&mut self, id: SectionId, len: u64) -> SectionLayout {
        let layout = SectionLayout::Allocated { address: self.layout.current_address.into() };

        self.layout.sections.insert(id, layout);
        self.layout.current_address += len;
        self.len += len;
        self.sections.push(id);

        layout
    }

    pub(crate) fn finalize(self, type_: SegmentType, perms: ElfPermissions) {
        self.layout.segments.push(Segment {
            start: self.start,
            len: self.len,
            align: PAGE_SIZE,
            type_,
            perms,
            sections: self.sections,
        });

        // Align to the page boundary.
        self.layout.current_address = (self.layout.current_address + PAGE_SIZE) & !(PAGE_SIZE - 1);
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub(crate) enum SectionLayout {
    Allocated { address: Address },
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
    Interpreter,
    Program,
    Uninitialized,
    Dynamic,
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
