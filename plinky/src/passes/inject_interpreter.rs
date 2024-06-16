use crate::cli::{CliOptions, Mode};
use crate::interner::intern;
use crate::repr::object::Object;
use crate::repr::sections::{DataSection, Section, SectionContent};
use plinky_diagnostics::ObjectSpan;
use plinky_elf::ids::serial::{SectionId, SerialIds};
use plinky_elf::{ElfDeduplication, ElfPermissions};

pub(crate) fn run(
    options: &CliOptions,
    ids: &mut SerialIds,
    object: &mut Object,
) -> Option<SectionId> {
    match object.mode {
        Mode::PositionDependent => return None,
        Mode::PositionIndependent => {}
    }

    let interpreter = match (&options.dynamic_linker, object.env.class) {
        (Some(linker), _) => linker.as_bytes().into(),
        (None, plinky_elf::ElfClass::Elf32) => b"/lib/ld-linux.so.2".into(),
        (None, plinky_elf::ElfClass::Elf64) => b"/lib64/ld-linux-x86-64.so.2".into(),
    };

    let id = ids.allocate_section_id();
    object.sections.add(Section {
        id,
        name: intern(".interp"),
        perms: ElfPermissions { read: true, write: false, execute: false },
        source: ObjectSpan::new_synthetic(),
        content: SectionContent::Data(DataSection {
            deduplication: ElfDeduplication::Disabled,
            bytes: interpreter,
            relocations: Vec::new(),
        }),
    });

    Some(id)
}
