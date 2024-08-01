use crate::cli::CliOptions;
use crate::passes;
use crate::passes::build_elf::ids::BuiltElfIds;
use crate::passes::build_elf::ElfBuilderError;
use crate::passes::deduplicate::DeduplicationError;
use crate::passes::gc_sections::RemovedSection;
use crate::passes::layout::Layout;
use crate::passes::load_inputs::LoadInputsError;
use crate::passes::relocate::RelocationError;
use crate::passes::replace_section_relative_symbols::ReplaceSectionRelativeSymbolsError;
use crate::passes::write_to_disk::WriteToDiskError;
use crate::repr::object::Object;
use plinky_elf::ids::serial::SerialIds;
use plinky_elf::ElfObject;
use plinky_macros::{Display, Error};
use crate::utils::before_freeze::BeforeFreeze;
use crate::passes::prepare_dynamic::PrepareDynamicError;

pub(crate) fn link_driver(
    options: &CliOptions,
    callbacks: &dyn LinkerCallbacks,
) -> Result<(), LinkerError> {
    let mut ids = SerialIds::new();

    // SAFETY: this is the beginning of the linking process.
    let before_freeze = unsafe { BeforeFreeze::new() };

    let mut object = passes::load_inputs::run(options, &mut ids, &before_freeze)?;
    passes::inject_symbol_table::run(&mut object, &mut ids);
    passes::prepare_dynamic::run(&options, &mut object, &mut ids)?;
    callbacks.on_inputs_loaded(&object);

    if options.gc_sections {
        let removed = passes::gc_sections::run(&mut object, &before_freeze);
        callbacks.on_sections_removed_by_gc(&object, &removed);
    }

    let deduplications = passes::deduplicate::run(&mut object, &mut ids, &before_freeze)?;

    passes::generate_got::generate_got(&mut ids, &mut object, &before_freeze);
    passes::exclude_section_symbols_from_tables::remove(&mut object, &before_freeze);
    passes::demote_global_hidden_symbols::run(&mut object);

    // We cannot change which symbols appear on symbol views from this point onwards, otherwise the
    // layout will be incorrect (as symbol tables will have a different size).
    drop(before_freeze);

    let layout = passes::layout::run(&mut object, deduplications);
    callbacks.on_layout_calculated(&object, &layout);

    passes::relocate::run(&mut object, &layout)?;
    callbacks.on_relocations_applied(&object, &layout);

    passes::replace_section_relative_symbols::replace(&mut object, &layout)?;

    let elf = passes::build_elf::run(object, layout, ids)?;
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
    Dynamic(PrepareDynamicError),
    #[transparent]
    RelocationFailed(RelocationError),
    #[transparent]
    ElfBuildFailed(ElfBuilderError),
    #[transparent]
    ReplaceSectionRelativeSymbolsFailed(ReplaceSectionRelativeSymbolsError),
    #[transparent]
    WriteToDiskFailed(WriteToDiskError),
}
