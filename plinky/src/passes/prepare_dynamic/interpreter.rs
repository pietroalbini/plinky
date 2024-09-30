use crate::cli::{CliOptions, DynamicLinker};
use crate::repr::object::Object;
use crate::repr::sections::DataSection;
use crate::repr::segments::{Segment, SegmentContent, SegmentType};
use plinky_elf::ids::serial::{SectionId, SerialIds};
use plinky_elf::{ElfClass, ElfPermissions};
use plinky_macros::{Display, Error};

pub(crate) fn run(
    options: &CliOptions,
    ids: &mut SerialIds,
    object: &mut Object,
) -> Result<Option<SectionId>, InjectInterpreterError> {
    let mut interpreter: Vec<u8> = match (&options.dynamic_linker, object.env.class) {
        (DynamicLinker::Unsupported, _) => return Ok(None),
        (DynamicLinker::Custom(linker), _) => linker.as_bytes().into(),
        (DynamicLinker::PlatformDefault, ElfClass::Elf32) => b"/lib/ld-linux.so.2".into(),
        (DynamicLinker::PlatformDefault, ElfClass::Elf64) => b"/lib64/ld-linux-x86-64.so.2".into(),
    };

    // The interpreter needs to be a null-terminated string, so ensure that there are no other byte
    // zeroes before adding our own at the end.
    if interpreter.iter().any(|&b| b == 0) {
        return Err(InjectInterpreterError::NullByteInInterpreter);
    }
    interpreter.push(0);

    let section = object
        .sections
        .builder(".interp", DataSection::new(ElfPermissions::R, &interpreter))
        .create(ids);

    object.segments.add(Segment {
        align: 1,
        type_: SegmentType::Interpreter,
        perms: ElfPermissions::R,
        content: vec![SegmentContent::Section(section)],
    });

    Ok(Some(section))
}

#[derive(Debug, Error, Display)]
pub(crate) enum InjectInterpreterError {
    #[display("unsupported null byte in the i")]
    NullByteInInterpreter,
}
