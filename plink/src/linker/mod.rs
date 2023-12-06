mod elf_builder;
pub(crate) mod layout;
pub(crate) mod object;
mod relocator;
mod strings;
mod symbols;

use crate::cli::CliOptions;
use crate::linker::elf_builder::{ElfBuilder, ElfBuilderContext, ElfBuilderError};
use crate::linker::layout::{LayoutCalculatorError, SectionLayout, SectionMerge};
use crate::linker::object::{Object, ObjectLoadError};
use crate::linker::relocator::RelocationError;
use plink_elf::errors::LoadError;
use plink_elf::ids::serial::SerialIds;
use plink_elf::{ElfEnvironment, ElfObject};
use plink_macros::Error;
use std::fs::File;
use std::path::PathBuf;
use crate::write_to_disk::{write_to_disk, WriteToDiskError};

pub(crate) fn link_driver(
    options: &CliOptions,
    callbacks: &dyn LinkerCallbacks,
) -> Result<(), LinkerError> {
    let mut object = Object::new();
    let mut ids = SerialIds::new();
    let mut first_environment: Option<EnvironmentAndPath> = None;

    for path in &options.inputs {
        let elf = ElfObject::load(
            &mut File::open(path)
                .map_err(|e| LinkerError::ReadElfFailed(path.into(), LoadError::IO(e)))?,
            &mut ids,
        )
        .map_err(|e| LinkerError::ReadElfFailed(path.into(), e))?;

        let new_env = EnvironmentAndPath {
            env: elf.env,
            path: path.into(),
        };
        match first_environment {
            Some(first_env) if first_env.env != new_env.env => {
                return Err(LinkerError::MismatchedEnv(first_env.clone(), new_env));
            }
            Some(_) => {}
            None => first_environment = Some(new_env),
        }

        object
            .merge_elf(elf)
            .map_err(|e| LinkerError::ObjectLoadFailed(path.into(), e))?;
    }
    callbacks.on_inputs_loaded(&object).result()?;

    let (mut object, section_merges) = object.calculate_layout()?;
    callbacks.on_layout_calculated(&object, &section_merges).result()?;

    // TODO: we need a better impl for this.
    let env = first_environment.ok_or(LinkerError::NoObjectLoaded)?;

    object.relocate()?;
    callbacks.on_relocations_applied(&object).result()?;

    let elf_builder = ElfBuilder::new(ElfBuilderContext {
        entrypoint: options.entry.clone(),
        env: env.env,
        object,
        section_merges,
    });
    let elf = elf_builder.build()?;
    callbacks.on_elf_built(&elf).result()?;

    write_to_disk(elf, &options.output)?;

    Ok(())
}

#[derive(Debug, Clone)]
pub(crate) struct EnvironmentAndPath {
    env: ElfEnvironment,
    path: PathBuf,
}

pub(crate) trait LinkerCallbacks {
    fn on_inputs_loaded(&self, _object: &Object<()>) -> CallbackOutcome {
        CallbackOutcome::Continue
    }

    fn on_layout_calculated(
        &self,
        _object: &Object<SectionLayout>,
        _merges: &[SectionMerge],
    ) -> CallbackOutcome {
        CallbackOutcome::Continue
    }

    fn on_relocations_applied(&self, _object: &Object<SectionLayout>) -> CallbackOutcome {
        CallbackOutcome::Continue
    }

    fn on_elf_built(&self, _elf: &ElfObject<SerialIds>) -> CallbackOutcome {
        CallbackOutcome::Continue
    }
}

#[must_use]
pub(crate) enum CallbackOutcome {
    Continue,
    Stop,
}

impl CallbackOutcome {
    fn result(self) -> Result<(), LinkerError> {
        match self {
            CallbackOutcome::Continue => Ok(()),
            CallbackOutcome::Stop => Err(LinkerError::CallbackEarlyExit),
        }
    }
}

#[derive(Debug, Error)]
pub(crate) enum LinkerError {
    CallbackEarlyExit,
    NoObjectLoaded,
    ReadElfFailed(PathBuf, #[source] LoadError),
    MismatchedEnv(EnvironmentAndPath, EnvironmentAndPath),
    ObjectLoadFailed(PathBuf, #[source] ObjectLoadError),
    LayoutCalculationFailed(#[from] LayoutCalculatorError),
    RelocationFailed(#[from] RelocationError),
    ElfBuildFailed(#[from] ElfBuilderError),
    WriteToDiskFailed(#[from] WriteToDiskError),
}

impl std::fmt::Display for LinkerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LinkerError::CallbackEarlyExit => f.write_str("early exit caused by a callback"),
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
            LinkerError::RelocationFailed(_) => f.write_str("failed to relocate the object"),
            LinkerError::NoObjectLoaded => f.write_str("no object loaded"),
            LinkerError::ElfBuildFailed(_) => f.write_str("failed to prepare the resulting object"),
            LinkerError::WriteToDiskFailed(_) => f.write_str("failed to write the linked object to disk"),
        }
    }
}
