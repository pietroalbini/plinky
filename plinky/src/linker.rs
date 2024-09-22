use crate::cli::CliOptions;
use crate::passes;
use crate::passes::build_elf::ids::BuiltElfIds;
use crate::passes::build_elf::ElfBuilderError;
use crate::passes::deduplicate::{Deduplication, DeduplicationError};
use crate::passes::gc_sections::RemovedSection;
use crate::passes::generate_got::GenerateGotError;
use crate::passes::load_inputs::LoadInputsError;
use crate::passes::prepare_dynamic::PrepareDynamicError;
use crate::passes::relocate::RelocationError;
use crate::passes::replace_section_relative_symbols::ReplaceSectionRelativeSymbolsError;
use crate::passes::write_to_disk::WriteToDiskError;
use crate::repr::object::Object;
use crate::utils::address_resolver::AddressResolver;
use plinky_elf::ids::serial::{SectionId, SerialIds};
use plinky_elf::writer::layout::{Layout, LayoutError};
use plinky_elf::ElfObject;
use plinky_macros::{Display, Error};
use std::collections::BTreeMap;

pub(crate) fn link_driver(
    options: &CliOptions,
    callbacks: &dyn LinkerCallbacks,
) -> Result<(), LinkerError> {
    let mut ids = SerialIds::new();

    let mut object = passes::load_inputs::run(options, &mut ids)?;
    passes::inject_symbol_table::run(&mut object, &mut ids);
    passes::inject_gnu_stack::run(&mut object);
    callbacks.on_inputs_loaded(&object);

    if options.gc_sections {
        let removed = passes::gc_sections::run(&mut object);
        callbacks.on_sections_removed_by_gc(&object, &removed);
    }

    let deduplications = passes::deduplicate::run(&mut object, &mut ids)?;

    let dynamic = passes::prepare_dynamic::run(&options, &mut object, &mut ids)?;
    passes::generate_got::generate_got(&options, &mut ids, &mut object, &dynamic)?;
    passes::generate_plt::run(&mut ids, &mut object);

    passes::exclude_section_symbols_from_tables::remove(&mut object);
    passes::demote_global_hidden_symbols::run(&mut object);
    passes::create_segments::run(&mut object);

    // This must be executed after we create all sections marked as data.inside_relro=true.
    passes::inject_gnu_relro::run(&mut object);

    let layout = passes::layout::run(&object)?;
    callbacks.on_layout_calculated(&object, &layout, &deduplications);

    let resolver = AddressResolver::new(&layout, &deduplications);

    passes::relocate::run(&mut object, &resolver)?;
    callbacks.on_relocations_applied(&object, &layout);

    passes::replace_section_relative_symbols::replace(&mut object, &resolver)?;

    let (elf, conversion_map) = passes::build_elf::run(object, &layout, &resolver)?;
    callbacks.on_elf_built(&elf);

    let layout = layout.convert_ids(&conversion_map);
    passes::write_to_disk::run(elf, layout, &options.output)?;

    Ok(())
}

pub(crate) trait LinkerCallbacks {
    fn on_inputs_loaded(&self, _object: &Object) {}

    fn on_sections_removed_by_gc(&self, _object: &Object, _removed: &[RemovedSection]) {}

    fn on_layout_calculated(
        &self,
        _object: &Object,
        _layout: &Layout<SerialIds>,
        _deduplications: &BTreeMap<SectionId, Deduplication>,
    ) {
    }

    fn on_relocations_applied(&self, _object: &Object, _layout: &Layout<SerialIds>) {}

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
    #[display("failed to generate the global offset table")]
    GenerateGot(#[from] GenerateGotError),
    #[transparent]
    RelocationFailed(RelocationError),
    #[transparent]
    LayoutError(LayoutError),
    #[transparent]
    ElfBuildFailed(ElfBuilderError),
    #[transparent]
    ReplaceSectionRelativeSymbolsFailed(ReplaceSectionRelativeSymbolsError),
    #[transparent]
    WriteToDiskFailed(WriteToDiskError),
}
