mod render_gc;
mod render_layout;
mod render_object;
mod utils;

use crate::cli::DebugPrint;
use crate::debug_print::render_gc::render_gc;
use crate::debug_print::render_layout::render_layout;
use crate::debug_print::render_object::render_object;
use crate::linker::LinkerCallbacks;
use crate::passes::gc_sections::RemovedSection;
use crate::passes::layout::Layout;
use crate::repr::object::Object;
use plinky_diagnostics::widgets::Text;
use plinky_diagnostics::{Diagnostic, DiagnosticKind};
use plinky_elf::ids::serial::SerialIds;
use plinky_elf::ElfObject;
use std::collections::BTreeSet;

pub(crate) struct DebugCallbacks {
    pub(crate) print: BTreeSet<DebugPrint>,
}

impl LinkerCallbacks for DebugCallbacks {
    fn on_inputs_loaded(&self, object: &Object) {
        if self.print.contains(&DebugPrint::LoadedObject) {
            render(render_object("loaded object", object, None));
        }
    }

    fn on_sections_removed_by_gc(&self, object: &Object, removed: &[RemovedSection]) {
        if self.print.contains(&DebugPrint::Gc) {
            render(render_gc(object, removed));
        }
    }

    fn on_layout_calculated(&self, object: &Object, layout: &Layout) {
        if self.print.contains(&DebugPrint::Layout) {
            render(render_layout(object, layout));
        }
    }

    fn on_relocations_applied(&self, object: &Object, layout: &Layout) {
        if self.print.contains(&DebugPrint::RelocatedObject) {
            render(render_object("object after relocations are applied", object, Some(layout)));
        }
    }

    fn on_elf_built(&self, elf: &ElfObject<SerialIds>) {
        if self.print.contains(&DebugPrint::FinalElf) {
            render(
                Diagnostic::new(DiagnosticKind::DebugPrint, "built elf")
                    .add(Text::new(format!("{elf:#x?}"))),
            );
        }
    }
}

fn render(diagnostic: Diagnostic) {
    eprintln!("{diagnostic}\n");
}
