mod object;

mod strings;
use crate::linker::object::{Object, ObjectLoadError};
use plink_elf::errors::LoadError;
use plink_elf::ids::serial::SerialIds;
use plink_elf::{ElfEnvironment, ElfObject};
use std::fs::File;
use std::path::{Path, PathBuf};

pub(crate) struct Linker {
    object: Object,
    ids: SerialIds,
    first_environment: Option<EnvironmentAndPath>,
}

impl Linker {
    pub(crate) fn new() -> Self {
        Linker {
            object: Object::new(),
            ids: SerialIds::new(),
            first_environment: None,
        }
    }

    pub(crate) fn load_file(&mut self, path: &Path) -> Result<(), LinkerError> {
        let object = ElfObject::load(
            &mut File::open(path)
                .map_err(|e| LinkerError::ReadElfFailed(path.into(), LoadError::IO(e)))?,
            &mut self.ids,
        )
        .map_err(|e| LinkerError::ReadElfFailed(path.into(), e))?;

        self.check_matching_environment(EnvironmentAndPath {
            env: object.env,
            path: path.into(),
        })?;

        self.object
            .merge_elf(object)
            .map_err(|e| LinkerError::ObjectLoadFailed(path.into(), e))?;

        Ok(())
    }

    fn check_matching_environment(
        &mut self,
        new_env: EnvironmentAndPath,
    ) -> Result<(), LinkerError> {
        match &self.first_environment {
            Some(first_env) => {
                if first_env.env != new_env.env {
                    return Err(LinkerError::MismatchedEnv(first_env.clone(), new_env));
                }
            }
            None => {
                self.first_environment = Some(new_env);
            }
        }
        Ok(())
    }

    pub(crate) fn loaded_object_for_debug_print(&self) -> &dyn std::fmt::Debug {
        &self.object
    }
}

#[derive(Debug, Clone)]
pub(crate) struct EnvironmentAndPath {
    env: ElfEnvironment,
    path: PathBuf,
}

#[derive(Debug)]
pub(crate) enum LinkerError {
    ReadElfFailed(PathBuf, LoadError),
    MismatchedEnv(EnvironmentAndPath, EnvironmentAndPath),
    ObjectLoadFailed(PathBuf, ObjectLoadError),
}

impl std::error::Error for LinkerError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            LinkerError::ReadElfFailed(_, err) => Some(err),
            LinkerError::MismatchedEnv(_, _) => None,
            LinkerError::ObjectLoadFailed(_, err) => Some(err),
        }
    }
}

impl std::fmt::Display for LinkerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LinkerError::ReadElfFailed(path, _) => {
                write!(f, "failed to read ELF file at {}", path.display())
            }
            LinkerError::MismatchedEnv(first, second) => {
                write!(
                    f,
                    "environment of {} is {:?}, while environment of {} is {:?}",
                    first.path.display(),
                    first.env,
                    second.path.display(),
                    second.env
                )
            }
            LinkerError::ObjectLoadFailed(path, _) => {
                write!(f, "failed to load ELF object at {}", path.display())
            }
        }
    }
}
