use crate::cli::{CliOptions, Mode};
use crate::interner::intern;
use crate::repr::object::Object;
use crate::repr::sections::{DataSection, Section, SectionContent};
use plinky_diagnostics::ObjectSpan;
use plinky_elf::ids::serial::{SectionId, SerialIds};
use plinky_elf::{ElfDeduplication, ElfPermissions};
use plinky_macros::{Display, Error};

pub(crate) fn run(
    options: &CliOptions,
    ids: &mut SerialIds,
    object: &mut Object,
) -> Result<Option<SectionId>, InjectInterpreterError> {
    match object.mode {
        Mode::PositionDependent => return Ok(None),
        Mode::PositionIndependent => {}
    }

    let mut interpreter: Vec<u8> = match (&options.dynamic_linker, object.env.class) {
        (Some(linker), _) => linker.as_bytes().into(),
        (None, plinky_elf::ElfClass::Elf32) => b"/lib/ld-linux.so.2".into(),
        (None, plinky_elf::ElfClass::Elf64) => b"/lib64/ld-linux-x86-64.so.2".into(),
    };

    // The interpreter needs to be a null-terminated string, so ensure that there are no other byte
    // zeroes before adding our own at the end.
    if interpreter.iter().any(|&b| b == 0) {
        return Err(InjectInterpreterError::NullByteInInterpreter);
    }
    interpreter.push(0);

    let id = ids.allocate_section_id();
    object.sections.add(Section {
        id,
        name: intern(".interp"),
        source: ObjectSpan::new_synthetic(),
        content: SectionContent::Data(DataSection {
            perms: ElfPermissions::empty().read(),
            deduplication: ElfDeduplication::Disabled,
            bytes: interpreter,
            relocations: Vec::new(),
        }),
    });

    Ok(Some(id))
}

#[derive(Debug, Error, Display)]
pub(crate) enum InjectInterpreterError {
    #[display("unsupported null byte in the i")]
    NullByteInInterpreter,
}
