use crate::passes::load_inputs::merge_elf::MergeElfError;
use crate::repr::object::Object;
use crate::repr::strings::Strings;
use crate::repr::symbols::Symbols;
use plink_elf::errors::LoadError;
use plink_elf::ids::serial::SerialIds;
use plink_elf::{ElfEnvironment, ElfObject};
use plink_macros::{Display, Error};
use std::collections::BTreeMap;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;

mod merge_elf;

pub(crate) fn run(paths: &[PathBuf], ids: &mut SerialIds) -> Result<Object<()>, LoadInputsError> {
    let mut state = None;

    for path in paths {
        let mut file = File::open(path).map_err(|e| LoadInputsError::OpenFailed(path.into(), e))?;
        let elf = ElfObject::load(&mut BufReader::new(&mut file), ids)
            .map_err(|e| LoadInputsError::ParseError(path.into(), e))?;

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
                    .map_err(|e| LoadInputsError::MergeFailed(path.clone(), e))?;
                state = Some((object, path));
            }
            Some((object, first_path)) => {
                if object.env != elf.env {
                    return Err(LoadInputsError::MismatchedEnv {
                        first_path: (*first_path).into(),
                        first_env: object.env,
                        current_path: path.into(),
                        current_env: elf.env,
                    });
                }
                merge_elf::merge(object, elf)
                    .map_err(|e| LoadInputsError::MergeFailed(path.clone(), e))?;
            }
        }
    }

    state.map(|(o, _)| o).ok_or(LoadInputsError::NoInputFiles)
}

#[derive(Debug, Error, Display)]
pub(crate) enum LoadInputsError {
    #[display("failed to open file {f0:?}")]
    OpenFailed(PathBuf, #[source] std::io::Error),
    #[display("failed to parse ELF file at {f0:?}")]
    ParseError(PathBuf, #[source] LoadError),
    #[display("no input files were provided")]
    NoInputFiles,
    #[display("failed to include the ELF file {f0:?}")]
    MergeFailed(PathBuf, #[source] MergeElfError),
    #[display("environment of {first_path:?} is {first_env:?}, while environment of {current_path:?} is {current_env:?}")]
    MismatchedEnv {
        first_path: PathBuf,
        first_env: ElfEnvironment,
        current_path: PathBuf,
        current_env: ElfEnvironment,
    },
}
