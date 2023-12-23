use crate::cli::CliOptions;
use crate::passes;
use crate::passes::build_elf::ElfBuilderError;
use crate::passes::deduplicate::DeduplicationError;
use crate::passes::load_inputs::LoadInputsError;
use crate::passes::relocate::RelocationError;
use crate::passes::write_to_disk::WriteToDiskError;
use crate::repr::object::{Object, SectionLayout};
use plink_elf::ids::serial::SerialIds;
use plink_elf::ElfObject;
use plink_macros::{Display, Error};

pub(crate) fn link_driver(
    options: &CliOptions,
    callbacks: &dyn LinkerCallbacks,
) -> Result<(), LinkerError> {
    let mut ids = SerialIds::new();

    let mut object = passes::load_inputs::run(&options.inputs, &mut ids)?;
    callbacks.on_inputs_loaded(&object).result()?;

    passes::deduplicate::run(&mut object, &mut ids)?;

    let mut object = passes::layout::run(object);
    callbacks.on_layout_calculated(&object).result()?;

    passes::relocate::run(&mut object)?;
    callbacks.on_relocations_applied(&object).result()?;

    let elf = passes::build_elf::run(object, options)?;
    callbacks.on_elf_built(&elf).result()?;

    passes::write_to_disk::run(elf, &options.output)?;

    Ok(())
}

pub(crate) trait LinkerCallbacks {
    fn on_inputs_loaded(&self, _object: &Object<()>) -> CallbackOutcome {
        CallbackOutcome::Continue
    }

    fn on_layout_calculated(&self, _object: &Object<SectionLayout>) -> CallbackOutcome {
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

#[derive(Debug, Display, Error)]
pub(crate) enum LinkerError {
    #[display("early exit caused by a callback")]
    CallbackEarlyExit,
    #[transparent]
    LoadInputsFailed(LoadInputsError),
    #[transparent]
    DeduplicationFailed(DeduplicationError),
    #[transparent]
    RelocationFailed(RelocationError),
    #[transparent]
    ElfBuildFailed(ElfBuilderError),
    #[transparent]
    WriteToDiskFailed(WriteToDiskError),
}
