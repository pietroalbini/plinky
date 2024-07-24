use crate::passes::layout::{Layout, SectionLayout};
use crate::utils::ints::Address;
use plinky_elf::ids::serial::SectionId;
use plinky_elf::ElfPermissions;

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Segment {
    pub(crate) align: u64,
    pub(crate) type_: SegmentType,
    pub(crate) perms: ElfPermissions,
    pub(crate) content: SegmentContent,
}

impl Segment {
    pub(crate) fn start(&self, layout: &Layout) -> SegmentStart {
        match &self.content {
            SegmentContent::ProgramHeader => SegmentStart::ProgramHeader,
            SegmentContent::ElfHeader => SegmentStart::Address(0u64.into()),
            SegmentContent::Sections(ids) => SegmentStart::Address(
                ids.iter()
                    .map(|id| match layout.of_section(*id) {
                        SectionLayout::Allocated { address } => *address,
                        SectionLayout::NotAllocated => panic!("non-allocated section in layout"),
                    })
                    .min()
                    .expect("empty segment"),
            ),
        }
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum SegmentType {
    ProgramHeader,
    Interpreter,
    Program,
    Uninitialized,
    Dynamic,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum SegmentContent {
    ProgramHeader,
    ElfHeader,
    Sections(Vec<SectionId>),
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub(crate) enum SegmentStart {
    ProgramHeader,
    Address(Address),
}
