use crate::cli::CliOptions;
use crate::passes;
use crate::passes::build_elf::ids::BuiltElfIds;
use crate::passes::build_elf::ElfBuilderError;
use crate::passes::deduplicate::DeduplicationError;
use crate::passes::gc_sections::RemovedSection;
use crate::passes::layout::Layout;
use crate::passes::load_inputs::LoadInputsError;
use crate::passes::relocate::RelocationError;
use crate::passes::write_to_disk::WriteToDiskError;
use crate::repr::object::Object;
use plinky_elf::ids::serial::SerialIds;
use plinky_elf::ElfObject;
use plinky_macros::{Display, Error};

pub(crate) fn link_driver(
    options: &CliOptions,
    callbacks: &dyn LinkerCallbacks,
) -> Result<(), LinkerError> {
    let mut ids = SerialIds::new();

    let mut object = passes::load_inputs::run(options, &mut ids)?;
    passes::inject_version::run(&mut ids, &mut object);
    callbacks.on_inputs_loaded(&object);

    if options.gc_sections {
        let removed = passes::gc_sections::run(&mut object);
        callbacks.on_sections_removed_by_gc(&object, &removed);
    }

    let deduplications = passes::deduplicate::run(&mut object, &mut ids)?;

    let layout = passes::layout::run(&object, deduplications);
    callbacks.on_layout_calculated(&object, &layout);

    passes::relocate::run(&mut object, &layout)?;
    callbacks.on_relocations_applied(&object, &layout);
    let elf = passes::build_elf::run(object, &layout)?;
    callbacks.on_elf_built(&elf);

    passes::write_to_disk::run(elf, &options.output)?;

    Ok(())
}

pub(crate) trait LinkerCallbacks {
    fn on_inputs_loaded(&self, _object: &Object) {}

    fn on_sections_removed_by_gc(&self, _object: &Object, _removed: &[RemovedSection]) {}

    fn on_layout_calculated(&self, _object: &Object, _layout: &Layout) {}

    fn on_relocations_applied(&self, _object: &Object, _layout: &Layout) {}

    fn on_elf_built(&self, _elf: &ElfObject<BuiltElfIds>) {}
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
