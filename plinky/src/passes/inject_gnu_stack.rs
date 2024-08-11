use crate::repr::object::Object;
use crate::repr::segments::{Segment, SegmentContent, SegmentType};
use plinky_elf::ElfPermissions;

pub(crate) fn run(object: &mut Object) {
    object.segments.add(Segment {
        align: 1,
        type_: SegmentType::GnuStack,
        perms: ElfPermissions {
            read: true,
            write: true,
            execute: object.executable_stack,
        },
        content: SegmentContent::Empty,
    });
}
