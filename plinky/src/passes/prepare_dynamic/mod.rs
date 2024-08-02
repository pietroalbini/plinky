use crate::cli::CliOptions;
use crate::passes::prepare_dynamic::interpreter::InjectInterpreterError;
use crate::repr::object::{DynamicEntry, Object};
use crate::repr::relocations::Relocation;
use crate::repr::sections::{
    RelocationsSection, SectionContent, StringsForSymbolsSection, SymbolsSection, SysvHashSection,
};
use crate::repr::segments::{Segment, SegmentContent, SegmentType};
use crate::repr::symbols::views::DynamicSymbolTable;
use plinky_elf::ids::serial::{SectionId, SerialIds};
use plinky_elf::ElfPermissions;
use plinky_macros::{Display, Error};

mod interpreter;

pub(crate) fn run(
    options: &CliOptions,
    object: &mut Object,
    ids: &mut SerialIds,
    got_relocations: Vec<Relocation>,
) -> Result<(), PrepareDynamicError> {
    interpreter::run(options, ids, object)?;

    let mut segment_content = Vec::new();
    let mut create =
        |name: &str, content: SectionContent, entry: fn(SectionId) -> DynamicEntry| -> SectionId {
            let id = object.sections.builder(name, content).create(ids);
            segment_content.push(id);
            object.dynamic_entries.push(entry(id));
            id
        };

    let string_table_id = create(
        ".dynstr",
        StringsForSymbolsSection::new(DynamicSymbolTable).into(),
        DynamicEntry::StringTable,
    );

    let symbol_table_id = create(
        ".dynsym",
        SymbolsSection::new(string_table_id, DynamicSymbolTable, true).into(),
        DynamicEntry::SymbolTable,
    );

    create(
        ".hash",
        SysvHashSection::new(DynamicSymbolTable, symbol_table_id).into(),
        DynamicEntry::Hash,
    );

    if !got_relocations.is_empty() {
        create(
            ".rela.got",
            RelocationsSection::new(None, symbol_table_id, got_relocations).into(),
            DynamicEntry::Rela,
        );
    }

    object.segments.push(Segment {
        align: 0x1000,
        type_: SegmentType::Program,
        perms: ElfPermissions::empty().read(),
        content: SegmentContent::Sections(segment_content),
    });

    Ok(())
}

#[derive(Debug, Error, Display)]
pub(crate) enum PrepareDynamicError {
    #[transparent]
    Interpreter(InjectInterpreterError),
}
