use crate::cli::Mode;
use crate::passes::deduplicate::Deduplication;
use crate::repr::object::Object;
use crate::repr::sections::SectionContent;
use crate::repr::segments::{Segment, SegmentContent, SegmentType};
use crate::utils::ints::{Address, Offset, OutOfBoundsError};
use plinky_elf::ids::serial::SectionId;
use plinky_elf::ElfPermissions;
use plinky_macros::{Display, Error};
use std::collections::BTreeMap;

const PAGE_SIZE: u64 = 0x1000;
const STATIC_BASE_ADDRESS: u64 = 0x400000;
const PIE_BASE_ADDRESS: u64 = PAGE_SIZE;

pub(crate) fn run(
    object: &mut Object,
    deduplications: BTreeMap<SectionId, Deduplication>,
    interp_section: Option<SectionId>,
) -> Layout {
    let mut grouped: BTreeMap<_, Vec<_>> = BTreeMap::new();
    let mut not_allocated = Vec::new();
    for section in object.sections.iter() {
        match &section.content {
            SectionContent::Data(data) => grouped
                .entry((
                    if Some(section.id) == interp_section {
                        SegmentType::Interpreter
                    } else {
                        SegmentType::Program
                    },
                    data.perms,
                ))
                .or_default()
                .push((section.id, data.bytes.len() as u64)),
            SectionContent::Uninitialized(uninit) => grouped
                .entry((SegmentType::Uninitialized, uninit.perms))
                .or_default()
                .push((section.id, uninit.len)),
            // Do not include these sections in the layout:
            SectionContent::StringsForSymbols(_) => not_allocated.push(section.id),
            SectionContent::Symbols(_) => not_allocated.push(section.id),
        }
    }

    let mut layout = Layout {
        current_address: match object.mode {
            Mode::PositionDependent => STATIC_BASE_ADDRESS,
            Mode::PositionIndependent => PIE_BASE_ADDRESS,
        },
        sections: BTreeMap::new(),
        deduplications,
    };
    for ((type_, perms), sections) in grouped.into_iter() {
        if perms.read || perms.write || perms.execute {
            let mut segment = layout.prepare_segment();
            for &(section, len) in &sections {
                segment.add_section(section, len);
            }
            segment.finalize(object, type_, perms);
        } else {
            // Avoid allocating sections that cannot be accessed at runtime.
            for (section, _) in sections {
                layout.sections.insert(section, SectionLayout::NotAllocated);
            }
        }
    }

    for id in not_allocated {
        layout.sections.insert(id, SectionLayout::NotAllocated);
    }

    layout
}

pub(crate) struct Layout {
    current_address: u64,
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

    pub(crate) fn prepare_segment(&mut self) -> PendingSegment {
        PendingSegment { start: self.current_address, sections: Vec::new(), layout: self }
    }
}

pub(crate) struct PendingSegment<'a> {
    layout: &'a mut Layout,
    sections: Vec<SectionId>,
    start: u64,
}

impl PendingSegment<'_> {
    pub(crate) fn add_section(&mut self, id: SectionId, len: u64) -> SectionLayout {
        let layout = SectionLayout::Allocated { address: self.layout.current_address.into() };

        self.layout.sections.insert(id, layout);
        self.layout.current_address += len;
        self.sections.push(id);

        layout
    }

    pub(crate) fn finalize(self, object: &mut Object, type_: SegmentType, perms: ElfPermissions) {
        object.segments.insert(Segment {
            start: self.start,
            align: PAGE_SIZE,
            type_,
            perms,
            content: SegmentContent::Sections(self.sections),
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

#[derive(Debug, Display, Error)]
pub(crate) enum AddressResolutionError {
    #[display("address points to section {f0:?}, which is not going to be allocated in memory")]
    PointsToUnallocatedSection(SectionId),
    #[display("referenced an offset not aligned to the deduplication boundaries")]
    UnalignedReferenceToDeduplication,
    #[transparent]
    OutOfBounds(OutOfBoundsError),
}
