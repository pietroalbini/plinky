use crate::cli::{CliOptions, Mode};
use crate::passes::prepare_dynamic::interpreter::InjectInterpreterError;
use crate::repr::object::{DynamicEntry, Object};
use crate::repr::sections::{StringsForSymbolsSection, SymbolsSection, SysvHashSection};
use crate::repr::segments::{Segment, SegmentContent, SegmentType};
use crate::repr::symbols::views::DynamicSymbolTable;
use plinky_elf::ids::serial::SerialIds;
use plinky_elf::ElfPermissions;
use plinky_macros::{Display, Error};

mod interpreter;

pub(crate) fn run(
    options: &CliOptions,
    object: &mut Object,
    ids: &mut SerialIds,
) -> Result<(), PrepareDynamicError> {
    match object.mode {
        Mode::PositionDependent => return Ok(()),
        Mode::PositionIndependent => {}
    }

    interpreter::run(options, ids, object)?;

    let string_table_id = ids.allocate_section_id();
    let symbol_table_id = ids.allocate_section_id();
    let hash_id = ids.allocate_section_id();

    object
        .sections
        .builder(".dynstr", StringsForSymbolsSection::new(DynamicSymbolTable))
        .create_with_id(string_table_id);

    object
        .sections
        .builder(".dynsym", SymbolsSection::new(string_table_id, DynamicSymbolTable, true))
        .create_with_id(symbol_table_id);

    object
        .sections
        .builder(".hash", SysvHashSection::new(DynamicSymbolTable, symbol_table_id))
        .create_with_id(hash_id);

    object.segments.push(Segment {
        align: 0x1000,
        type_: SegmentType::Program,
        perms: ElfPermissions::empty().read(),
        content: SegmentContent::Sections(vec![string_table_id, symbol_table_id, hash_id]),
    });

    object.dynamic_entries.push(DynamicEntry::StringTable(string_table_id));
    object.dynamic_entries.push(DynamicEntry::SymbolTable(symbol_table_id));
    object.dynamic_entries.push(DynamicEntry::Hash(hash_id));

    Ok(())
}

#[derive(Debug, Error, Display)]
pub(crate) enum PrepareDynamicError {
    #[transparent]
    Interpreter(InjectInterpreterError),
}
