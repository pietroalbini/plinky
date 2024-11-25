use crate::cli::{CliOptions, DynamicLinker, Mode};
use crate::interner::intern;
use crate::repr::dynamic_entries::DynamicEntry;
use crate::repr::object::Object;
use crate::repr::sections::{
    DataSection, DynamicSection, SectionContent, SectionId, StringsSection, SymbolsSection,
    SysvHashSection,
};
use crate::repr::segments::{Segment, SegmentContent, SegmentId, SegmentType};
use crate::repr::symbols::views::DynamicSymbolTable;
use crate::repr::symbols::{LoadSymbolsError, SymbolId, SymbolValue, UpcomingSymbol};
use plinky_elf::{ElfClass, ElfPermissions};
use plinky_macros::{Display, Error, Getters};
use plinky_utils::ints::Offset;
use plinky_utils::raw_types::RawTypeAsPointerSize;

pub(crate) fn run(
    options: &CliOptions,
    object: &mut Object,
) -> Result<Option<DynamicContext>, GenerateDynamicError> {
    match object.mode {
        Mode::PositionDependent => return Ok(None),
        Mode::PositionIndependent => {}
        Mode::SharedLibrary => {}
    };

    let mut segment_content = Vec::new();

    segment_content.push(SegmentContent::ElfHeader);
    segment_content.push(SegmentContent::ProgramHeader);

    match object.mode {
        Mode::PositionDependent => unreachable!(),
        Mode::PositionIndependent => {
            let interpreter = add_interpreter(options, object);
            segment_content.push(SegmentContent::Section(interpreter?));
        }
        Mode::SharedLibrary => {}
    }

    let mut dynstr_section = StringsSection::new(DynamicSymbolTable);
    if let Some(soname) = &options.shared_object_name {
        let soname_id = dynstr_section.add_custom_string(soname);
        object.dynamic_entries.add(DynamicEntry::SharedObjectName(soname_id));
    }
    for input in &object.inputs {
        if input.shared_object {
            let name = input.span.to_string(); // FIXME: this is wrong
            let needed_id = dynstr_section.add_custom_string(name);
            object.dynamic_entries.add(DynamicEntry::Needed(needed_id));
        }
    }

    let mut create =
        |name: &str, content: SectionContent, entry: fn(SectionId) -> DynamicEntry| -> SectionId {
            let id = object.sections.builder(name, content).create();
            segment_content.push(SegmentContent::Section(id));
            object.dynamic_entries.add(entry(id));
            id
        };
    let dynstr = create(".dynstr", dynstr_section.into(), DynamicEntry::StringTable);

    let dynsym = create(
        ".dynsym",
        SymbolsSection::new(dynstr, DynamicSymbolTable, true).into(),
        DynamicEntry::SymbolTable,
    );

    create(".hash", SysvHashSection::new(DynamicSymbolTable, dynsym).into(), DynamicEntry::Hash);

    let dynamic = object.sections.builder(".dynamic", DynamicSection::new(dynstr)).create();
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

    let dynamic_symbol = object
        .symbols
        .add(UpcomingSymbol::GlobalHidden {
            name: intern("_DYNAMIC"),
            value: SymbolValue::SectionRelative { section: dynamic, offset: Offset::from(0) },
        })
        .map_err(GenerateDynamicError::DynamicSymbolCreation)?;

    Ok(Some(DynamicContext { dynsym, segment: dynamic_segment, dynamic_symbol }))
}

fn add_interpreter(
    options: &CliOptions,
    object: &mut Object,
) -> Result<SectionId, GenerateDynamicError> {
    let mut interpreter: Vec<u8> = match (&options.dynamic_linker, object.env.class) {
        (DynamicLinker::Custom(linker), _) => linker.as_bytes().into(),
        (DynamicLinker::PlatformDefault, ElfClass::Elf32) => b"/lib/ld-linux.so.2".into(),
        (DynamicLinker::PlatformDefault, ElfClass::Elf64) => b"/lib64/ld-linux-x86-64.so.2".into(),
    };

    // The interpreter needs to be a null-terminated string, so ensure that there are no other byte
    // zeroes before adding our own at the end.
    if interpreter.iter().any(|&b| b == 0) {
        return Err(GenerateDynamicError::NullByteInInterpreter);
    }
    interpreter.push(0);

    let section = object
        .sections
        .builder(".interp", DataSection::new(ElfPermissions::R, &interpreter))
        .create();

    object.segments.add(Segment {
        align: 1,
        type_: SegmentType::Interpreter,
        perms: ElfPermissions::R,
        content: vec![SegmentContent::Section(section)],
    });

    Ok(section)
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
    #[display("the --dynamic-linker flag contained a null byte")]
    NullByteInInterpreter,
}
