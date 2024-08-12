use plinky_elf::ids::serial::{SectionId, SerialIds};
use plinky_elf::writer::layout::{Layout, Part, PartMetadata};
use plinky_elf::ElfPermissions;

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
    pub(crate) fn layout(&self, layout: &Layout<SerialIds>) -> PartMetadata {
        match &self.content {
            SegmentContent::ProgramHeader => layout.metadata(&Part::ProgramHeaders).clone(),
            SegmentContent::ElfHeader => layout.metadata(&Part::Header).clone(),
            SegmentContent::Sections(sections) => {
                let mut sections_iter = sections.iter();
                let mut metadata = layout
                    .metadata_of_section(
                        sections_iter.next().expect("at least one section must be present"),
                    )
                    .clone();
                for section in sections_iter {
                    metadata = metadata
                        .add(layout.metadata_of_section(section))
                        .expect("sections are not positioned in the layout correctly");
                }
                metadata
            }
            SegmentContent::Empty => PartMetadata::EMPTY,
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
    GnuStack,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum SegmentContent {
    ProgramHeader,
    ElfHeader,
    Sections(Vec<SectionId>),
    Empty,
}
