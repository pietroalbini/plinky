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

    pub(crate) fn add(&mut self, segment: Segment) -> SegmentId {
        let id = SegmentId(self.segments.len());
        self.segments.push(segment);
        id
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (SegmentId, &Segment)> {
        self.segments.iter().enumerate().map(|(idx, segment)| (SegmentId(idx), segment))
    }

    pub(crate) fn get_mut(&mut self, id: SegmentId) -> &mut Segment {
        &mut self.segments[id.0]
    }

    pub(crate) fn len(&self) -> usize {
        self.segments.len()
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub(crate) struct SegmentId(usize);

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Segment {
    pub(crate) align: u64,
    pub(crate) type_: SegmentType,
    pub(crate) perms: ElfPermissions,
    pub(crate) content: Vec<SegmentContent>,
}

impl Segment {
    pub(crate) fn layout(&self, layout: &Layout<SerialIds>) -> PartMetadata {
        let mut content = self.content.iter();
        let Some(first) = content.next() else { return PartMetadata::EMPTY };

        let mut metadata = first.layout(layout);
        for part in content {
            metadata = metadata
                .add(&part.layout(layout))
                .expect("the content of the section is not positioned correctly");
        }

        metadata
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
    GnuRelro,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum SegmentContent {
    ProgramHeader,
    ElfHeader,
    Section(SectionId),
}

impl SegmentContent {
    fn layout(&self, layout: &Layout<SerialIds>) -> PartMetadata {
        match self {
            SegmentContent::ProgramHeader => layout.metadata(&Part::ProgramHeaders).clone(),
            SegmentContent::ElfHeader => layout.metadata(&Part::Header).clone(),
            SegmentContent::Section(section) => layout.metadata_of_section(&section).clone(),
        }
    }
}
