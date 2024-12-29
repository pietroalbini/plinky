use crate::repr::object::Object;
use crate::repr::sections::SectionContent;
use crate::repr::segments::{Segment, SegmentContent, SegmentType};
use plinky_elf::ElfPermissions;
use std::collections::{BTreeMap, BTreeSet};

const PAGE_SIZE: u64 = 0x1000;

pub(crate) fn run(object: &mut Object) {
    // Segments can be created before this step. Ensure we don't put the sections in
    // them in two different segments.
    let sections_already_in_segments = object
        .segments
        .iter()
        .filter(|(_id, segment)| segment.type_ == SegmentType::Program)
        .flat_map(|(_id, segment)| {
            segment.content.iter().filter_map(|c| match c {
                SegmentContent::ProgramHeader => None,
                SegmentContent::ElfHeader => None,
                SegmentContent::Section(id) => Some(*id),
            })
        })
        .collect::<BTreeSet<_>>();

    let mut segments = BTreeMap::new();
    for section in object.sections.iter() {
        if sections_already_in_segments.contains(&section.id) {
            continue;
        }
        let (type_, perms, align) = match &section.content {
            SectionContent::Data(data) => (SegmentType::Program, data.perms, PAGE_SIZE),
            SectionContent::Uninitialized(uninit) => {
                (SegmentType::Uninitialized, uninit.perms, PAGE_SIZE)
            }
            SectionContent::Notes(notes) => {
                (SegmentType::Notes, ElfPermissions::R, notes.alignment(object.env.class))
            }

            SectionContent::Strings(_)
            | SectionContent::Symbols(_)
            | SectionContent::SysvHash(_)
            | SectionContent::GnuHash(_)
            | SectionContent::Relocations(_)
            | SectionContent::Dynamic(_)
            | SectionContent::SectionNames => continue,
        };
        if perms.read || perms.write || perms.execute {
            segments
                .entry((type_, perms, align))
                .or_insert_with(Vec::new)
                .push(SegmentContent::Section(section.id));
        }
    }

    for ((type_, perms, align), content) in segments {
        object.segments.add(Segment { align, type_, perms, content });
    }
}
