use crate::cli::Mode;
use crate::interner::intern;
use crate::repr::dynamic_entries::DynamicEntry;
use crate::repr::object::Object;
use crate::repr::sections::{
    DynamicSection, SectionContent, StringsSection, SymbolsSection, SysvHashSection,
};
use crate::repr::segments::{Segment, SegmentContent, SegmentId, SegmentType};
use crate::repr::symbols::views::DynamicSymbolTable;
use crate::repr::symbols::{LoadSymbolsError, Symbol, SymbolValue};
use plinky_elf::ids::serial::{SectionId, SerialIds, SymbolId};
use plinky_elf::ElfPermissions;
use plinky_macros::{Display, Error, Getters};
use plinky_utils::ints::Offset;
use plinky_utils::raw_types::RawTypeAsPointerSize;

pub(crate) fn run(
    object: &mut Object,
    ids: &mut SerialIds,
) -> Result<Option<DynamicContext>, GenerateDynamicError> {
    match object.mode {
        Mode::PositionDependent => return Ok(None),
        Mode::PositionIndependent => {}
        Mode::SharedLibrary => {}
    };

    let mut segment_content = Vec::new();

    segment_content.push(SegmentContent::ElfHeader);
    segment_content.push(SegmentContent::ProgramHeader);

    let mut create =
        |name: &str, content: SectionContent, entry: fn(SectionId) -> DynamicEntry| -> SectionId {
            let id = object.sections.builder(name, content).create(ids);
            segment_content.push(SegmentContent::Section(id));
            object.dynamic_entries.add(entry(id));
            id
        };

    let dynstr = create(
        ".dynstr",
        StringsSection::new(DynamicSymbolTable).into(),
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
        perms: ElfPermissions::R,
        content: segment_content,
    });

    object.segments.add(Segment {
        align: <u64 as RawTypeAsPointerSize>::size(object.env.class) as _,
        type_: SegmentType::Dynamic,
        perms: ElfPermissions::R,
        content: vec![SegmentContent::Section(dynamic)],
    });

    object.segments.add(Segment {
        align: 0x1000,
        type_: SegmentType::ProgramHeader,
        perms: ElfPermissions::R,
        content: vec![SegmentContent::ProgramHeader],
    });

    match object.mode {
        Mode::PositionDependent => unreachable!(),
        Mode::PositionIndependent => object.dynamic_entries.flags1.pie = true,
        Mode::SharedLibrary => {}
    }

    let dynamic_symbol = ids.allocate_symbol_id();
    object
        .symbols
        .add_symbol(Symbol::new_global_hidden(
            dynamic_symbol,
            intern("_DYNAMIC"),
            SymbolValue::SectionRelative { section: dynamic, offset: Offset::from(0) },
        ))
        .map_err(GenerateDynamicError::DynamicSymbolCreation)?;

    Ok(Some(DynamicContext { dynsym, segment: dynamic_segment, dynamic_symbol }))
}

#[derive(Debug, Getters)]
pub(crate) struct DynamicContext {
    #[get]
    dynsym: SectionId,
    #[get]
    segment: SegmentId,
    #[get]
    dynamic_symbol: SymbolId,
}

#[derive(Debug, Error, Display)]
pub(crate) enum GenerateDynamicError {
    #[display("failed to prepare the _DYNAMIC symbol")]
    DynamicSymbolCreation(#[source] LoadSymbolsError),
}
