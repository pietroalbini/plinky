use plinky_utils::ints::Address;
use plinky_elf::ids::serial::{SectionId, SerialIds};
use plinky_elf::ElfPermissions;
use plinky_elf::writer::layout::Layout;

#[derive(Debug)]
pub(crate) struct Segments {
    segments: Vec<Segment>,
}

impl Segments {
    pub(crate) fn new() -> Self {
        Self { segments: Vec::new() }
    }

    pub(crate) fn add(&mut self, segment: Segment) {
        self.segments.push(segment);
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = &Segment> {
        self.segments.iter()
    }

    pub(crate) fn len(&self) -> usize {
        self.segments.len()
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Segment {
    pub(crate) align: u64,
    pub(crate) type_: SegmentType,
    pub(crate) perms: ElfPermissions,
    pub(crate) content: SegmentContent,
}

impl Segment {
    pub(crate) fn start(&self, layout: &Layout<SerialIds>) -> SegmentStart {
        match &self.content {
            SegmentContent::ProgramHeader => SegmentStart::ProgramHeader,
            SegmentContent::ElfHeader => SegmentStart::Address(0u64.into()),
            SegmentContent::Sections(ids) => SegmentStart::Address(
                ids.iter()
                    .map(|id| match &layout.metadata_of_section(id).memory {
                        Some(mem) => mem.address,
                        None => panic!("non-allocated section {id:?} in layout"),
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
