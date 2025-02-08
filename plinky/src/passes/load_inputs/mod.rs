use crate::cli::CliOptions;
use crate::interner::intern;
use crate::passes::load_inputs::merge_elf::{MergeElfError, merge_elf};
use crate::passes::load_inputs::read_objects::{NextObject, ObjectsReader, ReadObjectsError};
use crate::passes::load_inputs::section_groups::{SectionGroups, SectionGroupsForObject};
use crate::passes::load_inputs::shared_objects::{SharedObjectError, load_shared_object};
use crate::passes::load_inputs::strings::Strings;
use crate::repr::dynamic_entries::DynamicEntries;
use crate::repr::object::Object;
use crate::repr::sections::{SectionContent, Sections};
use crate::repr::segments::Segments;
use crate::repr::symbols::{LoadSymbolsError, Symbols, UpcomingSymbol};
use plinky_diagnostics::ObjectSpan;
use plinky_elf::{ElfEnvironment, LoadError};
use plinky_macros::{Display, Error};

mod cleanup;
mod inject_version;
mod merge_elf;
mod read_objects;
mod section_groups;
mod shared_objects;
mod strings;

pub(crate) fn run(options: &CliOptions) -> Result<Object, LoadInputsError> {
    let mut reader = ObjectsReader::new(&options.search_paths, &options.inputs);

    let mut empty_symbols = Symbols::new().map_err(LoadInputsError::SymbolTableCreationFailed)?;
    let entry_point = options
        .entry
        .as_ref()
        .map(|entry| empty_symbols.add(UpcomingSymbol::GlobalUnknown { name: intern(entry) }))
        .transpose()
        .map_err(LoadInputsError::EntryInsertionFailed)?;

    let mut state = State::Empty {
        symbols: empty_symbols,
        strings: Strings::new(),
        section_groups: SectionGroups::new(),
    };
    loop {
        let symbols = match &state {
            State::Empty { symbols, .. } => symbols,
            State::WithContent { object, .. } => &object.symbols,
        };
        let Some(next) = reader.next_object(symbols)? else { break };
        let source = next.source.clone();

        state = match state {
            State::Empty { symbols, mut section_groups, mut strings } => {
                let mut object = Object {
                    env: next.reader.env(),
                    inputs: Vec::new(),
                    sections: Sections::new(),
                    segments: Segments::new(),
                    symbols,
                    dynamic_entries: DynamicEntries::new(),
                    got: None,
                    got_plt: None,
                    plt: None,
                    entry_point,
                    mode: options.mode,
                    executable_stack: options.executable_stack,
                    gnu_stack_section_ignored: false,
                };

                object.sections.builder(".shstrtab", SectionContent::SectionNames).create();

                load_object(&mut object, &mut strings, section_groups.for_object(), next)?;
                State::WithContent { object, strings, section_groups, first_span: source }
            }
            State::WithContent { mut object, mut strings, mut section_groups, first_span } => {
                if object.env != next.reader.env() {
                    return Err(LoadInputsError::MismatchedEnv {
                        first_span: first_span.clone(),
                        first_env: object.env,
                        current_span: source,
                        current_env: next.reader.env(),
                    });
                }
                load_object(&mut object, &mut strings, section_groups.for_object(), next)?;
                State::WithContent { object, strings, section_groups, first_span }
            }
        }
    }

    match state {
        State::Empty { .. } => Err(LoadInputsError::NoInputFiles),
        State::WithContent { mut object, .. } => {
            inject_version::run(&mut object);
            cleanup::run(&mut object);
            Ok(object)
        }
    }
}

fn load_object(
    object: &mut Object,
    strings: &mut Strings,
    section_groups: SectionGroupsForObject<'_>,
    NextObject { mut reader, source, library_name, options }: NextObject,
) -> Result<(), LoadInputsError> {
    match reader.type_() {
        plinky_elf::ElfType::Relocatable => {
            let elf = reader
                .into_object()
                .map_err(|e| LoadInputsError::ParseFailed(source.clone(), e))?;
            merge_elf(object, strings, section_groups, elf, source.clone())
                .map_err(|e| LoadInputsError::MergeFailed(source, e))
        }
        plinky_elf::ElfType::SharedObject => {
            load_shared_object(object, &mut reader, &library_name, options, source.clone())
                .map_err(|e| LoadInputsError::SharedLoadFailed(source, e))
        }
        plinky_elf::ElfType::Executable => Err(LoadInputsError::ExecutableUnsupported(source)),
        plinky_elf::ElfType::Core => Err(LoadInputsError::CoreUnsupported(source)),
    }
}

enum State {
    Empty {
        symbols: Symbols,
        strings: Strings,
        section_groups: SectionGroups,
    },
    WithContent {
        object: Object,
        strings: Strings,
        section_groups: SectionGroups,
        first_span: ObjectSpan,
    },
}

#[derive(Debug, Error, Display)]
pub(crate) enum LoadInputsError {
    #[display("no input files were provided")]
    NoInputFiles,
    #[display("core dump {f0} is not supported as linker input")]
    CoreUnsupported(ObjectSpan),
    #[display("executable {f0} is not supported as linker input")]
    ExecutableUnsupported(ObjectSpan),
    #[display("failed to add the entry point as an unknown symbol")]
    EntryInsertionFailed(#[source] LoadSymbolsError),
    #[transparent]
    ReadFailed(ReadObjectsError),
    #[display("failed to parse {f0}")]
    ParseFailed(ObjectSpan, #[source] LoadError),
    #[display("failed to include the ELF file {f0}")]
    MergeFailed(ObjectSpan, #[source] MergeElfError),
    #[display("failed to load the shared object {f0}")]
    SharedLoadFailed(ObjectSpan, #[source] SharedObjectError),
    #[display("failed to create the symbol table")]
    SymbolTableCreationFailed(#[source] LoadSymbolsError),
    #[display(
        "environment of {first_span} is {first_env:?}, while environment of {current_span} is {current_env:?}"
    )]
    MismatchedEnv {
        first_span: ObjectSpan,
        first_env: ElfEnvironment,
        current_span: ObjectSpan,
        current_env: ElfEnvironment,
    },
}
