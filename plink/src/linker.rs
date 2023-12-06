use crate::cli::CliOptions;
use crate::passes;
use crate::passes::load_inputs::LoadInputsError;
use crate::repr::elf_builder::{ElfBuilder, ElfBuilderContext, ElfBuilderError};
use crate::repr::layout::{LayoutCalculatorError, SectionLayout, SectionMerge};
use crate::repr::object::Object;
use crate::repr::relocator::RelocationError;
use crate::write_to_disk::{write_to_disk, WriteToDiskError};
use plink_elf::ids::serial::SerialIds;
use plink_elf::ElfObject;
use plink_macros::Error;

pub(crate) fn link_driver(
    options: &CliOptions,
    callbacks: &dyn LinkerCallbacks,
) -> Result<(), LinkerError> {
    let mut ids = SerialIds::new();

    let object = passes::load_inputs::run(&options.inputs, &mut ids)?;
    callbacks.on_inputs_loaded(&object).result()?;

    let (mut object, section_merges) = object.calculate_layout()?;
    callbacks
        .on_layout_calculated(&object, &section_merges)
        .result()?;

    object.relocate()?;
    callbacks.on_relocations_applied(&object).result()?;

    let elf_builder = ElfBuilder::new(ElfBuilderContext {
        entrypoint: options.entry.clone(),
        object,
        section_merges,
    });
    let elf = elf_builder.build()?;
    callbacks.on_elf_built(&elf).result()?;

    write_to_disk(elf, &options.output)?;

    Ok(())
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
    LoadInputsFailed(#[from] LoadInputsError),
    LayoutCalculationFailed(#[from] LayoutCalculatorError),
    RelocationFailed(#[from] RelocationError),
    ElfBuildFailed(#[from] ElfBuilderError),
    WriteToDiskFailed(#[from] WriteToDiskError),
}

impl std::fmt::Display for LinkerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LinkerError::CallbackEarlyExit => f.write_str("early exit caused by a callback"),
            LinkerError::LoadInputsFailed(_) => f.write_str("failed to load input files"),
            LinkerError::LayoutCalculationFailed(_) => {
                f.write_str("failed to calculate the resulting layout")
            }
            LinkerError::RelocationFailed(_) => f.write_str("failed to relocate the object"),
            LinkerError::ElfBuildFailed(_) => f.write_str("failed to prepare the resulting object"),
            LinkerError::WriteToDiskFailed(_) => {
                f.write_str("failed to write the linked object to disk")
            }
        }
    }
}
