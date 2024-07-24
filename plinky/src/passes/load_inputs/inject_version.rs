use crate::repr::object::Object;
use crate::repr::sections::DataSection;
use plinky_elf::ids::serial::SerialIds;
use plinky_elf::{ElfDeduplication, ElfPermissions};

pub(crate) fn run(ids: &mut SerialIds, object: &mut Object) {
    let mut data = DataSection::new(ElfPermissions::empty(), b"Linker: plinky\0");
    data.deduplication = ElfDeduplication::ZeroTerminatedStrings;

    object.sections.builder(".comment", data).create(ids);
}
