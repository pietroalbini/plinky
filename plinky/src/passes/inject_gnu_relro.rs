use crate::repr::object::Object;
use crate::repr::sections::SectionContent;
use crate::repr::segments::{Segment, SegmentContent, SegmentType};
use plinky_elf::ElfPermissions;

pub(crate) fn run(object: &mut Object) {
    let mut content = Vec::new();
    for section in object.sections.iter() {
        let SectionContent::Data(data) = &section.content else { continue };
        if data.inside_relro {
            content.push(SegmentContent::Section(section.id));
        }
    }

    if !content.is_empty() {
        object.segments.add(Segment {
            align: 0x1,
            type_: SegmentType::GnuRelro,
            perms: ElfPermissions::R,
            content,
        });
    }
}
