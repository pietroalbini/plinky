mod layout;
mod object;
mod strings;
mod symbols;

use crate::linker::layout::{LayoutCalculatorError, SectionLayout, SectionMerge};
use crate::linker::object::{Object, ObjectLoadError};
use plink_elf::errors::LoadError;
use plink_elf::ids::serial::{SerialIds, SectionId};
use plink_elf::{ElfEnvironment, ElfObject};
use std::fs::File;
use std::path::{Path, PathBuf};
use std::collections::BTreeMap;

pub(crate) struct Linker<S: LinkerStage> {
    object: Object<S::LayoutInformation>,
    stage: S,
}

impl Linker<InitialStage> {
    pub(crate) fn new() -> Self {
        Linker {
            object: Object::new(),
            stage: InitialStage {
                ids: SerialIds::new(),
                first_environment: None,
            },
        }
    }

    pub(crate) fn load_file(&mut self, path: &Path) -> Result<(), LinkerError> {
        let object = ElfObject::load(
            &mut File::open(path)
                .map_err(|e| LinkerError::ReadElfFailed(path.into(), LoadError::IO(e)))?,
            &mut self.stage.ids,
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
        match &self.stage.first_environment {
            Some(first_env) => {
                if first_env.env != new_env.env {
                    return Err(LinkerError::MismatchedEnv(first_env.clone(), new_env));
                }
            }
            None => {
                self.stage.first_environment = Some(new_env);
            }
        }
        Ok(())
    }

    pub(crate) fn loaded_object_for_debug_print(&self) -> &dyn std::fmt::Debug {
        &self.object
    }

    pub(crate) fn calculate_layout(self) -> Result<Linker<LayoutStage>, LinkerError> {
        let (object, section_merges) = self.object.calculate_layout()?;
        Ok(Linker {
            object,
            stage: LayoutStage { section_merges },
        })
    }
}

impl Linker<LayoutStage> {
    pub(crate) fn section_addresses_for_debug_print(&self) -> impl std::fmt::Debug {
        self.object.section_addresses_for_debug_print()
    }

    pub(crate) fn section_merges_for_debug_print(&self) -> &[impl std::fmt::Debug] {
        &self.stage.section_merges
    }
}

pub(crate) trait LinkerStage {
    type LayoutInformation;
}

pub(crate) struct InitialStage {
    ids: SerialIds,
    first_environment: Option<EnvironmentAndPath>,
}

impl LinkerStage for InitialStage {
    type LayoutInformation = ();
}

pub(crate) struct LayoutStage {
    section_merges: Vec<SectionMerge>,
}

impl LinkerStage for LayoutStage {
    type LayoutInformation = SectionLayout;
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
    LayoutCalculationFailed(LayoutCalculatorError),
}

impl std::error::Error for LinkerError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            LinkerError::ReadElfFailed(_, err) => Some(err),
            LinkerError::MismatchedEnv(_, _) => None,
            LinkerError::ObjectLoadFailed(_, err) => Some(err),
            LinkerError::LayoutCalculationFailed(err) => Some(err),
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
            LinkerError::LayoutCalculationFailed(_) => {
                f.write_str("failed to calculate the resulting layout")
            }
        }
    }
}

impl From<LayoutCalculatorError> for LinkerError {
    fn from(value: LayoutCalculatorError) -> Self {
        LinkerError::LayoutCalculationFailed(value)
    }
}
