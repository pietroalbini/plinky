use crate::passes::load_inputs::merge_elf::MergeElfError;
use crate::passes::load_inputs::read_objects::{objects_iter, ReadObjectsError};
use crate::repr::object::Object;
use crate::repr::strings::Strings;
use crate::repr::symbols::Symbols;
use plink_elf::ids::serial::SerialIds;
use plink_elf::ElfEnvironment;
use plink_macros::{Display, Error};
use std::collections::BTreeMap;
use std::path::PathBuf;

mod merge_elf;
mod read_objects;

pub(crate) fn run(paths: &[PathBuf], ids: &mut SerialIds) -> Result<Object<()>, LoadInputsError> {
    let mut state = None;

    for result in objects_iter(paths, ids) {
        let (location, elf) = result?;

        match &mut state {
            None => {
                let mut object = Object {
                    env: elf.env,
                    sections: BTreeMap::new(),
                    section_ids_to_names: BTreeMap::new(),
                    strings: Strings::new(),
                    symbols: Symbols::new(),
                };
                merge_elf::merge(&mut object, elf)
                    .map_err(|e| LoadInputsError::MergeFailed(location.clone(), e))?;
                state = Some((object, location));
            }
            Some((object, first_location)) => {
                if object.env != elf.env {
                    return Err(LoadInputsError::MismatchedEnv {
                        first_location: first_location.clone(),
                        first_env: object.env,
                        current_location: location,
                        current_env: elf.env,
                    });
                }
                merge_elf::merge(object, elf)
                    .map_err(|e| LoadInputsError::MergeFailed(location.clone(), e))?;
            }
        }
    }

    state.map(|(o, _)| o).ok_or(LoadInputsError::NoInputFiles)
}

#[derive(Debug, Clone)]
pub(crate) enum ObjectLocation {
    File(PathBuf),
    Archive { archive: PathBuf, member: String },
}

impl std::fmt::Display for ObjectLocation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ObjectLocation::File(file) => write!(f, "{}", file.display()),
            ObjectLocation::Archive { archive, member } => {
                write!(f, "{member} inside archive {}", archive.display())
            }
        }
    }
}

#[derive(Debug, Error, Display)]
pub(crate) enum LoadInputsError {
    #[display("no input files were provided")]
    NoInputFiles,
    #[transparent]
    ReadFailed(ReadObjectsError),
    #[display("failed to include the ELF file {f0}")]
    MergeFailed(ObjectLocation, #[source] MergeElfError),
    #[display("environment of {current_location} is {first_env:?}, while environment of {current_location} is {current_env:?}")]
    MismatchedEnv {
        first_location: ObjectLocation,
        first_env: ElfEnvironment,
        current_location: ObjectLocation,
        current_env: ElfEnvironment,
    },
}
