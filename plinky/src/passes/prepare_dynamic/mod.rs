use crate::cli::{CliOptions, Mode};
use crate::passes::prepare_dynamic::interpreter::InjectInterpreterError;
use crate::repr::dynamic_entries::DynamicEntry;
use crate::repr::object::Object;
use crate::repr::sections::{
    DynamicSection, SectionContent, StringsForSymbolsSection, SymbolsSection, SysvHashSection,
};
use crate::repr::segments::{Segment, SegmentContent, SegmentId, SegmentType};
use crate::repr::symbols::views::DynamicSymbolTable;
use plinky_elf::ids::serial::{SectionId, SerialIds};
use plinky_elf::ElfPermissions;
use plinky_macros::{Display, Error, Getters};
use plinky_utils::raw_types::RawTypeAsPointerSize;

mod interpreter;

pub(crate) fn run(
    options: &CliOptions,
    object: &mut Object,
    ids: &mut SerialIds,
) -> Result<Option<DynamicContext>, PrepareDynamicError> {
    match object.mode {
        Mode::PositionDependent => return Ok(None),
        Mode::PositionIndependent => {}
    };

    let interpreter_section = interpreter::run(options, ids, object)?;

    let mut segment_content = Vec::new();

    segment_content.push(SegmentContent::ElfHeader);
    segment_content.push(SegmentContent::ProgramHeader);
    segment_content.push(SegmentContent::Section(interpreter_section));

    let mut create =
        |name: &str, content: SectionContent, entry: fn(SectionId) -> DynamicEntry| -> SectionId {
            let id = object.sections.builder(name, content).create(ids);
            segment_content.push(SegmentContent::Section(id));
            object.dynamic_entries.add(entry(id));
            id
        };

    let dynstr = create(
        ".dynstr",
        StringsForSymbolsSection::new(DynamicSymbolTable).into(),
        DynamicEntry::StringTable,
    );

    let dynsym = create(
        ".dynsym",
        SymbolsSection::new(dynstr, DynamicSymbolTable, true).into(),
        DynamicEntry::SymbolTable,
    );

    create(".hash", SysvHashSection::new(DynamicSymbolTable, dynsym).into(), DynamicEntry::Hash);

    let dynamic = object.sections.builder(".dynamic", DynamicSection::new(dynstr)).create(ids);
    segment_content.push(SegmentContent::Section(dynamic));

    let dynamic_segment = object.segments.add(Segment {
        align: 0x1000,
        type_: SegmentType::Program,
        perms: ElfPermissions::empty().read(),
        content: segment_content,
    });

    object.segments.add(Segment {
        align: <u64 as RawTypeAsPointerSize>::size(object.env.class) as _,
        type_: SegmentType::Dynamic,
        perms: ElfPermissions::empty().read(),
        content: vec![SegmentContent::Section(dynamic)],
    });

    object.segments.add(Segment {
        align: 0x1000,
        type_: SegmentType::ProgramHeader,
        perms: ElfPermissions::empty().read(),
        content: vec![SegmentContent::ProgramHeader],
    });

    match object.mode {
        Mode::PositionDependent => unreachable!(),
        Mode::PositionIndependent => object.dynamic_entries.add(DynamicEntry::PieFlag),
    }

    if options.read_only_after_relocations {
        object.segments.add(Segment {
            align: 0x1,
            type_: SegmentType::GnuRelro,
            perms: ElfPermissions::empty().read(),
            content: vec![SegmentContent::RelroSections],
        });
    }

    Ok(Some(DynamicContext { dynsym, segment: dynamic_segment }))
}

#[derive(Debug, Getters)]
pub(crate) struct DynamicContext {
    #[get]
    dynsym: SectionId,
    #[get]
    segment: SegmentId,
}

#[derive(Debug, Error, Display)]
pub(crate) enum PrepareDynamicError {
    #[transparent]
    Interpreter(InjectInterpreterError),
}
