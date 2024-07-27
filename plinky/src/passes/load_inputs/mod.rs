use crate::cli::CliOptions;
use crate::passes::load_inputs::merge_elf::MergeElfError;
use crate::passes::load_inputs::read_objects::{ObjectsReader, ReadObjectsError};
use crate::passes::load_inputs::section_groups::SectionGroups;
use crate::passes::load_inputs::strings::Strings;
use crate::repr::object::Object;
use crate::repr::sections::Sections;
use crate::repr::symbols::{LoadSymbolsError, Symbols};
use crate::utils::before_freeze::BeforeFreeze;
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

pub(crate) fn run(
    options: &CliOptions,
    ids: &mut SerialIds,
    before_freeze: &BeforeFreeze,
) -> Result<Object, LoadInputsError> {
    let mut reader = ObjectsReader::new(&options.inputs);

    let mut empty_symbols = Symbols::new(ids);
    let entry_point = empty_symbols
        .add_unknown_global(ids, &options.entry, before_freeze)
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
                    segments: Vec::new(),
                    symbols,
                    dynamic_relocations: Vec::new(),
                    got: None,
                    entry_point,
                    mode: options.mode,
                    executable_stack: options.executable_stack,
                    gnu_stack_section_ignored: false,
                };
                inject_version::run(ids, &mut object);
                merge_elf::merge(
                    &mut object,
                    &mut strings,
                    section_groups.for_object(),
                    source.clone(),
                    elf,
                    before_freeze,
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
                    before_freeze,
                )
                .map_err(|e| LoadInputsError::MergeFailed(source, e))?;
                State::WithContent { object, strings, section_groups, first_span }
            }
        }
    }

    match state {
        State::Empty { .. } => Err(LoadInputsError::NoInputFiles),
        State::WithContent { mut object, section_groups, .. } => {
            cleanup::run(&mut object, &section_groups, before_freeze);
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
    #[display("environment of {first_span} is {first_env:?}, while environment of {current_span} is {current_env:?}")]
    MismatchedEnv {
        first_span: ObjectSpan,
        first_env: ElfEnvironment,
        current_span: ObjectSpan,
        current_env: ElfEnvironment,
    },
}
