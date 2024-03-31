use crate::cli::CliOptions;
use crate::passes::load_inputs::merge_elf::MergeElfError;
use crate::passes::load_inputs::read_objects::{ObjectsReader, ReadObjectsError};
use crate::repr::object::{Object, Sections};
use crate::repr::strings::Strings;
use crate::repr::symbols::{LoadSymbolsError, Symbols};
use plinky_diagnostics::ObjectSpan;
use plinky_elf::ids::serial::SerialIds;
use plinky_elf::ElfEnvironment;
use plinky_macros::{Display, Error};

mod merge_elf;
mod read_objects;

pub(crate) fn run(options: &CliOptions, ids: &mut SerialIds) -> Result<Object, LoadInputsError> {
    let mut reader = ObjectsReader::new(&options.inputs);

    let mut empty_symbols = Symbols::new();
    let entry_point = empty_symbols
        .add_unknown_global(ids, &options.entry)
        .map_err(LoadInputsError::EntryInsertionFailed)?;

    let mut state = State::Empty { symbols: empty_symbols };
    loop {
        let symbols = match &state {
            State::Empty { symbols } => symbols,
            State::WithContent { object, .. } => &object.symbols,
        };
        let Some((source, elf)) = reader.next_object(ids, symbols)? else { break };

        state = match state {
            State::Empty { symbols } => {
                let mut object = Object {
                    env: elf.env,
                    sections: Sections::new(),
                    strings: Strings::new(),
                    symbols,
                    entry_point,
                    executable_stack: options.executable_stack,
                };
                merge_elf::merge(ids, &mut object, source.clone(), elf)
                    .map_err(|e| LoadInputsError::MergeFailed(source.clone(), e))?;
                State::WithContent { object, first_span: source }
            }
            State::WithContent { mut object, first_span } => {
                if object.env != elf.env {
                    return Err(LoadInputsError::MismatchedEnv {
                        first_span: first_span.clone(),
                        first_env: object.env,
                        current_span: source,
                        current_env: elf.env,
                    });
                }
                merge_elf::merge(ids, &mut object, source.clone(), elf)
                    .map_err(|e| LoadInputsError::MergeFailed(source, e))?;
                State::WithContent { object, first_span }
            }
        }
    }

    match state {
        State::Empty { .. } => Err(LoadInputsError::NoInputFiles),
        State::WithContent { object, .. } => Ok(object),
    }
}

enum State {
    Empty { symbols: Symbols },
    WithContent { object: Object, first_span: ObjectSpan },
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
