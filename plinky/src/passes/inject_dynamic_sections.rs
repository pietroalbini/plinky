use crate::cli::Mode;
use crate::repr::object::{DynamicEntry, Object};
use crate::repr::sections::{StringsForSymbolsSection, SymbolsSection};
use crate::repr::segments::{Segment, SegmentContent, SegmentType};
use crate::repr::symbols::views::DynamicSymbolTable;
use plinky_elf::ids::serial::SerialIds;
use plinky_elf::ElfPermissions;

pub(crate) fn run(object: &mut Object, ids: &mut SerialIds) {
    match object.mode {
        Mode::PositionDependent => return,
        Mode::PositionIndependent => {}
    }

    let string_table_id = ids.allocate_section_id();
    let symbol_table_id = ids.allocate_section_id();

    object
        .sections
        .builder(".dynstr", StringsForSymbolsSection::new(DynamicSymbolTable))
        .create_with_id(string_table_id);

    object
        .sections
        .builder(".dynsym", SymbolsSection::new(string_table_id, DynamicSymbolTable, true))
        .create_with_id(symbol_table_id);

    object.segments.push(Segment {
        align: 0x1000,
        type_: SegmentType::Program,
        perms: ElfPermissions::empty().read(),
        content: SegmentContent::Sections(vec![string_table_id, symbol_table_id]),
    });

    object.dynamic_entries.push(DynamicEntry::StringTable(string_table_id));
    object.dynamic_entries.push(DynamicEntry::SymbolTable(symbol_table_id));
}
