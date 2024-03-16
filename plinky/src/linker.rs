use crate::cli::CliOptions;
use crate::passes;
use crate::passes::build_elf::ElfBuilderError;
use crate::passes::deduplicate::DeduplicationError;
use crate::passes::load_inputs::LoadInputsError;
use crate::passes::relocate::RelocationError;
use crate::passes::write_to_disk::WriteToDiskError;
use crate::repr::object::{Object, SectionLayout};
use plinky_elf::ids::serial::SerialIds;
use plinky_elf::ElfObject;
use plinky_macros::{Display, Error};

pub(crate) fn link_driver(
    options: &CliOptions,
    callbacks: &dyn LinkerCallbacks,
) -> Result<(), LinkerError> {
    let mut ids = SerialIds::new();

    let mut object = passes::load_inputs::run(options, &mut ids)?;
    callbacks.on_inputs_loaded(&object);

    passes::deduplicate::run(&mut object, &mut ids)?;

    let mut object = passes::layout::run(object);
    callbacks.on_layout_calculated(&object);

    passes::relocate::run(&mut object)?;
    callbacks.on_relocations_applied(&object);

    let elf = passes::build_elf::run(object, options)?;
    callbacks.on_elf_built(&elf);

    passes::write_to_disk::run(elf, &options.output)?;

    Ok(())
}

pub(crate) trait LinkerCallbacks {
    fn on_inputs_loaded(&self, _object: &Object<()>) {}

    fn on_layout_calculated(&self, _object: &Object<SectionLayout>) {}

    fn on_relocations_applied(&self, _object: &Object<SectionLayout>) {}

    fn on_elf_built(&self, _elf: &ElfObject<SerialIds>) {}
}

#[derive(Debug, Display, Error)]
pub(crate) enum LinkerError {
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