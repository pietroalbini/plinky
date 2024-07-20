use plinky_elf::ids::serial::SectionId;
use plinky_elf::ElfPermissions;
use std::cmp::Ordering;

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Segment {
    pub(crate) start: u64,
    pub(crate) align: u64,
    pub(crate) type_: SegmentType,
    pub(crate) perms: ElfPermissions,
    pub(crate) content: SegmentContent,
}

impl PartialOrd for Segment {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Segment {
    fn cmp(&self, other: &Self) -> Ordering {
        self.type_
            .cmp(&other.type_)
            .then(self.start.cmp(&other.start))
            .then(self.content.cmp(&other.content))
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
