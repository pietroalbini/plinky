use crate::cli::{CliOptions, Mode};
use crate::repr::object::Object;
use crate::repr::sections::DataSection;
use crate::repr::segments::{Segment, SegmentContent, SegmentType};
use plinky_elf::ids::serial::SerialIds;
use plinky_elf::ElfPermissions;
use plinky_macros::{Display, Error};

pub(crate) fn run(
    options: &CliOptions,
    ids: &mut SerialIds,
    object: &mut Object,
) -> Result<(), InjectInterpreterError> {
    match object.mode {
        Mode::PositionDependent => return Ok(()),
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

    let section = object
        .sections
        .builder(".interp", DataSection::new(ElfPermissions::empty().read(), &interpreter))
        .create(ids);

    object.segments.add(Segment {
        align: 1,
        type_: SegmentType::Interpreter,
        perms: ElfPermissions::empty().read(),
        content: SegmentContent::Sections(vec![section]),
    });

    Ok(())
}

#[derive(Debug, Error, Display)]
pub(crate) enum InjectInterpreterError {
    #[display("unsupported null byte in the i")]
    NullByteInInterpreter,
}
