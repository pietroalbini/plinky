use crate::repr::object::Object;
use crate::repr::sections::SectionContent;
use crate::repr::segments::{Segment, SegmentContent, SegmentType};
use std::collections::{BTreeMap, BTreeSet};

const PAGE_SIZE: u64 = 0x1000;

pub(crate) fn run(object: &mut Object) {
    // Segments can be created before this step. Ensure we don't put the sections in
    // them in two different segments.
    let sections_already_in_segments = object
        .segments
        .iter()
        .filter_map(|segment| match &segment.content {
            SegmentContent::ProgramHeader => None,
            SegmentContent::ElfHeader => None,
            SegmentContent::Sections(sections) => Some(sections),
        })
        .flatten()
        .collect::<BTreeSet<_>>();

    let mut segments = BTreeMap::new();
    for section in object.sections.iter() {
        if sections_already_in_segments.contains(&section.id) {
            continue;
        }
        let (type_, perms) = match &section.content {
            SectionContent::Data(data) => (SegmentType::Program, data.perms),
            SectionContent::Uninitialized(uninit) => (SegmentType::Uninitialized, uninit.perms),

            SectionContent::StringsForSymbols(_)
            | SectionContent::Symbols(_)
            | SectionContent::SysvHash(_)
            | SectionContent::Relocations(_)
            | SectionContent::Dynamic(_) => continue,
        };
        if perms.read || perms.write || perms.execute {
            segments.entry((type_, perms)).or_insert_with(Vec::new).push(section.id);
        }
    }

    for ((type_, perms), sections) in segments {
        object.segments.add(Segment {
            align: PAGE_SIZE,
            type_,
            perms,
            content: SegmentContent::Sections(sections),
        });
    }
}
