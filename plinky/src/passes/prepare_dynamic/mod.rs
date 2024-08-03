use crate::cli::{CliOptions, Mode};
use crate::passes::prepare_dynamic::interpreter::InjectInterpreterError;
use crate::repr::object::{DynamicEntry, Object};
use crate::repr::relocations::Relocation;
use crate::repr::sections::{
    DynamicSection, RelocationsSection, SectionContent, StringsForSymbolsSection, SymbolsSection,
    SysvHashSection,
};
use crate::repr::segments::{Segment, SegmentContent, SegmentType};
use crate::repr::symbols::views::DynamicSymbolTable;
use crate::utils::before_freeze::BeforeFreeze;
use plinky_elf::ids::serial::{SectionId, SerialIds};
use plinky_elf::ElfPermissions;
use plinky_macros::{Display, Error};
use plinky_utils::raw_types::RawTypeAsPointerSize;

mod interpreter;

pub(crate) fn run(
    options: &CliOptions,
    object: &mut Object,
    ids: &mut SerialIds,
    got_relocations: Vec<Relocation>,
    before_freeze: &BeforeFreeze,
) -> Result<(), PrepareDynamicError> {
    interpreter::run(options, ids, object, before_freeze)?;

    let mut segment_content = Vec::new();
    let mut create =
        |name: &str, content: SectionContent, entry: fn(SectionId) -> DynamicEntry| -> SectionId {
            let id = object.sections.builder(name, content).create(ids);
            segment_content.push(id);
            object.dynamic_entries.push(entry(id));
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

    if !got_relocations.is_empty() {
        create(
            ".rela.got",
            RelocationsSection::new(None, dynsym, got_relocations).into(),
            DynamicEntry::Rela,
        );
    }

    let dynamic = object.sections.builder(".dynamic", DynamicSection::new(dynstr)).create(ids);
    segment_content.push(dynamic);

    object.segments.add(
        Segment {
            align: 0x1000,
            type_: SegmentType::Program,
            perms: ElfPermissions::empty().read(),
            content: SegmentContent::Sections(segment_content),
        },
        before_freeze,
    );

    object.segments.add(
        Segment {
            align: <u64 as RawTypeAsPointerSize>::size(object.env.class) as _,
            type_: SegmentType::Dynamic,
            perms: ElfPermissions::empty().read(),
            content: SegmentContent::Sections(vec![dynamic]),
        },
        before_freeze,
    );

    for type_ in [SegmentType::Program, SegmentType::ProgramHeader] {
        object.segments.add(
            Segment {
                align: 0x1000,
                type_,
                perms: ElfPermissions::empty().read(),
                content: SegmentContent::ProgramHeader,
            },
            before_freeze,
        );
    }

    object.segments.add(
        Segment {
            align: 0x1000,
            type_: SegmentType::Program,
            perms: ElfPermissions::empty().read(),
            content: SegmentContent::ElfHeader,
        },
        before_freeze,
    );

    match object.mode {
        Mode::PositionDependent => unreachable!(),
        Mode::PositionIndependent => object.dynamic_entries.push(DynamicEntry::PieFlag),
    }

    Ok(())
}

#[derive(Debug, Error, Display)]
pub(crate) enum PrepareDynamicError {
    #[transparent]
    Interpreter(InjectInterpreterError),
}
