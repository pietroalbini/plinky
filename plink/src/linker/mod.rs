use plink_elf::errors::LoadError;
use plink_elf::ids::serial::SerialIds;
use plink_elf::{ElfEnvironment, ElfObject};
use std::fs::File;
use std::path::{Path, PathBuf};

pub(crate) struct Linker {
    ids: SerialIds,
    first_environment: Option<EnvironmentAndPath>,
}

impl Linker {
    pub(crate) fn new() -> Self {
        Linker {
            ids: SerialIds::new(),
            first_environment: None,
        }
    }

    pub(crate) fn load_file(&mut self, path: &Path) -> Result<(), LinkerError> {
        let object = ElfObject::load(
            &mut File::open(path)
                .map_err(|e| LinkerError::LoadElfFailed(path.into(), LoadError::IO(e)))?,
            &mut self.ids,
        )
        .map_err(|e| LinkerError::LoadElfFailed(path.into(), e))?;

        self.check_matching_environment(EnvironmentAndPath {
            env: object.env,
            path: path.into(),
        })?;

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
}

#[derive(Debug, Clone)]
pub(crate) struct EnvironmentAndPath {
    env: ElfEnvironment,
    path: PathBuf,
}

#[derive(Debug)]
pub(crate) enum LinkerError {
    LoadElfFailed(PathBuf, LoadError),
    MismatchedEnv(EnvironmentAndPath, EnvironmentAndPath),
}

impl std::error::Error for LinkerError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            LinkerError::LoadElfFailed(_, err) => Some(err),
            LinkerError::MismatchedEnv(_, _) => None,
        }
    }
}

impl std::fmt::Display for LinkerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LinkerError::LoadElfFailed(path, _) => {
                write!(f, "failed to load ELF file at {}", path.display())
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
        }
    }
}
