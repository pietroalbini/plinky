use crate::cli::CliOptions;
use crate::passes::load_inputs::merge_elf::MergeElfError;
use crate::passes::load_inputs::read_objects::{ObjectsReader, ReadObjectsError};
use crate::repr::object::Object;
use crate::repr::strings::Strings;
use crate::repr::symbols::Symbols;
use plinky_diagnostics::ObjectSpan;
use plinky_elf::ids::serial::SerialIds;
use plinky_elf::ElfEnvironment;
use plinky_macros::{Display, Error};
use std::collections::BTreeMap;

mod merge_elf;
mod read_objects;

pub(crate) fn run(
    options: &CliOptions,
    ids: &mut SerialIds,
) -> Result<Object<()>, LoadInputsError> {
    let mut state: Option<(Object<()>, ObjectSpan)> = None;
    let mut reader = ObjectsReader::new(&options.inputs, ids);

    let mut empty_symbols = Symbols::new();
    empty_symbols.add_unknown_global(&options.entry);

    loop {
        let symbols = match &state {
            Some((object, _)) => &object.symbols,
            None => &empty_symbols,
        };
        let Some((source, elf)) = reader.next_object(symbols)? else { break };

        match &mut state {
            None => {
                let mut object = Object {
                    env: elf.env,
                    sections: BTreeMap::new(),
                    section_ids_to_names: BTreeMap::new(),
                    strings: Strings::new(),
                    symbols: Symbols::new(),
                };
                merge_elf::merge(&mut object, source.clone(), elf)
                    .map_err(|e| LoadInputsError::MergeFailed(source.clone(), e))?;
                state = Some((object, source));
            }
            Some((object, first_source)) => {
                if object.env != elf.env {
                    return Err(LoadInputsError::MismatchedEnv {
                        first_span: first_source.clone(),
                        first_env: object.env,
                        current_span: source,
                        current_env: elf.env,
                    });
                }
                merge_elf::merge(object, source.clone(), elf)
                    .map_err(|e| LoadInputsError::MergeFailed(source, e))?;
            }
        }
    }

    state.map(|(o, _)| o).ok_or(LoadInputsError::NoInputFiles)
}

#[derive(Debug, Error, Display)]
pub(crate) enum LoadInputsError {
    #[display("no input files were provided")]
    NoInputFiles,
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
