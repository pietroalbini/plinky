use crate::cli::CliOptions;
use crate::interner::intern;
use crate::passes::load_inputs::merge_elf::MergeElfError;
use crate::passes::load_inputs::read_objects::{ObjectsReader, ReadObjectsError};
use crate::passes::load_inputs::section_groups::SectionGroups;
use crate::passes::load_inputs::strings::Strings;
use crate::repr::dynamic_entries::DynamicEntries;
use crate::repr::object::Object;
use crate::repr::sections::{SectionContent, Sections};
use crate::repr::segments::Segments;
use crate::repr::symbols::{LoadSymbolsError, Symbols, UpcomingSymbol};
use plinky_diagnostics::ObjectSpan;
use plinky_elf::ids::serial::SerialIds;
use plinky_elf::ElfEnvironment;
use plinky_macros::{Display, Error};

mod cleanup;
mod inject_version;
mod merge_elf;
mod read_objects;
mod section_groups;
mod strings;

pub(crate) fn run(options: &CliOptions, ids: &mut SerialIds) -> Result<Object, LoadInputsError> {
    let shstrtab_id = ids.allocate_section_id();

    let mut reader = ObjectsReader::new(&options.inputs);

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
        let Some((source, elf)) = reader.next_object(ids, symbols)? else { break };

        state = match state {
            State::Empty { symbols, mut section_groups, mut strings } => {
                let mut object = Object {
                    env: elf.env,
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

                inject_version::run(ids, &mut object);
                object
                    .sections
                    .builder(".shstrtab", SectionContent::SectionNames)
                    .create_with_id(shstrtab_id);

                merge_elf::merge(
                    &mut object,
                    &mut strings,
                    section_groups.for_object(),
                    source.clone(),
                    elf,
                )
                .map_err(|e| LoadInputsError::MergeFailed(source.clone(), e))?;
                State::WithContent { object, strings, section_groups, first_span: source }
            }
            State::WithContent { mut object, mut strings, mut section_groups, first_span } => {
                if object.env != elf.env {
                    return Err(LoadInputsError::MismatchedEnv {
                        first_span: first_span.clone(),
                        first_env: object.env,
                        current_span: source,
                        current_env: elf.env,
                    });
                }
                merge_elf::merge(
                    &mut object,
                    &mut strings,
                    section_groups.for_object(),
                    source.clone(),
                    elf,
                )
                .map_err(|e| LoadInputsError::MergeFailed(source, e))?;
                State::WithContent { object, strings, section_groups, first_span }
            }
        }
    }

    match state {
        State::Empty { .. } => Err(LoadInputsError::NoInputFiles),
        State::WithContent { mut object, section_groups, .. } => {
            cleanup::run(&mut object, &section_groups);
            Ok(object)
        }
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
    #[display("failed to add the entry point as an unknown symbol")]
    EntryInsertionFailed(#[source] LoadSymbolsError),
    #[transparent]
    ReadFailed(ReadObjectsError),
    #[display("failed to include the ELF file {f0}")]
    MergeFailed(ObjectSpan, #[source] MergeElfError),
    #[display("failed to create the symbol table")]
    SymbolTableCreationFailed(#[source] LoadSymbolsError),
    #[display("environment of {first_span} is {first_env:?}, while environment of {current_span} is {current_env:?}")]
    MismatchedEnv {
        first_span: ObjectSpan,
        first_env: ElfEnvironment,
        current_span: ObjectSpan,
        current_env: ElfEnvironment,
    },
}
