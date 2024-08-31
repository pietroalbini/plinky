use super::sections::SectionContent;
use crate::repr::object::Object;
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
    pub(crate) fn layout(&self, object: &Object, layout: &Layout<SerialIds>) -> PartMetadata {
        let mut content = self.content.iter();
        let Some(first) = content.next() else { return PartMetadata::EMPTY };

        let mut metadata = first.layout(object, layout);
        for part in content {
            metadata = metadata
                .add(&part.layout(object, layout))
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
    RelroSections,
}

impl SegmentContent {
    fn layout(&self, object: &Object, layout: &Layout<SerialIds>) -> PartMetadata {
        match self {
            SegmentContent::ProgramHeader => layout.metadata(&Part::ProgramHeaders).clone(),
            SegmentContent::ElfHeader => layout.metadata(&Part::Header).clone(),
            SegmentContent::Section(section) => layout.metadata_of_section(&section).clone(),

            SegmentContent::RelroSections => {
                let mut sections = object.sections.iter().filter_map(|s| match &s.content {
                    SectionContent::Data(data) if data.inside_relro => Some(s.id),
                    _ => None,
                });
                let Some(first) = sections.next() else { return PartMetadata::EMPTY };

                let mut metadata = layout.metadata_of_section(&first).clone();
                for section in sections {
                    metadata = metadata
                        .add(&layout.metadata_of_section(&section))
                        .expect("relro sections are not adjacent");
                }
                metadata
            }
        }
    }
}
