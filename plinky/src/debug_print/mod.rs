pub(crate) mod filters;
mod render_gc;
mod render_layout;
mod render_object;
mod utils;

use crate::cli::DebugPrint;
use crate::debug_print::render_gc::render_gc;
use crate::debug_print::render_layout::render_layout;
use crate::debug_print::render_object::render_object;
use crate::linker::LinkerCallbacks;
use crate::passes::build_elf::ids::BuiltElfIds;
use crate::passes::deduplicate::Deduplication;
use crate::passes::gc_sections::RemovedSection;
use crate::repr::object::Object;
use plinky_diagnostics::{Diagnostic, DiagnosticKind};
use plinky_elf::ids::serial::{SectionId, SerialIds};
use plinky_elf::ElfObject;
use std::collections::{BTreeMap, BTreeSet};
use plinky_elf::writer::layout::Layout;

pub(crate) struct DebugCallbacks {
    pub(crate) print: BTreeSet<DebugPrint>,
}

impl LinkerCallbacks for DebugCallbacks {
    fn on_inputs_loaded(&self, object: &Object) {
        for print in &self.print {
            if let DebugPrint::LoadedObject(filters) = print {
                render(render_object("loaded object", filters, object, None));
            }
        }
    }

    fn on_sections_removed_by_gc(&self, object: &Object, removed: &[RemovedSection]) {
        if self.print.contains(&DebugPrint::Gc) {
            render(render_gc(object, removed));
        }
    }

    fn on_layout_calculated(
        &self,
        object: &Object,
        layout: &Layout<SerialIds>,
        deduplications: &BTreeMap<SectionId, Deduplication>,
    ) {
        if self.print.contains(&DebugPrint::Layout) {
            render(render_layout(object, layout, deduplications));
        }
    }

    fn on_relocations_applied(&self, object: &Object, layout: &Layout<SerialIds>) {
        for print in &self.print {
            if let DebugPrint::RelocatedObject(filters) = print {
                render(render_object(
                    "object after relocations are applied",
                    filters,
                    object,
                    Some(layout),
                ));
            }
        }
    }

    fn on_elf_built(&self, elf: &ElfObject<BuiltElfIds>) {
        for print in &self.print {
            if let DebugPrint::FinalElf(filters) = print {
                render(
                    Diagnostic::new(DiagnosticKind::DebugPrint, "built elf")
                        .add(plinky_elf::render_elf::render(elf, filters)),
                );
            }
        }
    }
}

fn render(diagnostic: Diagnostic) {
    eprintln!("{diagnostic}\n");
}
