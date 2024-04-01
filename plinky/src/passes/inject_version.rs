use crate::interner::intern;
use crate::repr::object::Object;
use crate::repr::sections::{DataSection, Section, SectionContent};
use plinky_diagnostics::ObjectSpan;
use plinky_elf::ids::serial::SerialIds;
use plinky_elf::{ElfDeduplication, ElfPermissions};

pub(crate) fn run(ids: &mut SerialIds, object: &mut Object) {
    object.sections.add(Section {
        id: ids.allocate_section_id(),
        name: intern(".comment"),
        perms: ElfPermissions { read: false, write: false, execute: false },
        source: ObjectSpan::new_synthetic(),
        content: SectionContent::Data(DataSection {
            deduplication: ElfDeduplication::ZeroTerminatedStrings,
            bytes: b"Linker: plinky\0".into(),
            relocations: Vec::new(),
        }),
    });
}
