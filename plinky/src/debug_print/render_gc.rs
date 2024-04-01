use crate::debug_print::utils::section_name;
use crate::passes::gc_sections::RemovedSection;
use crate::repr::object::Object;
use plinky_diagnostics::widgets::Table;
use plinky_diagnostics::{Diagnostic, DiagnosticKind};

pub(super) fn render_gc(object: &Object, removed: &[RemovedSection]) -> Diagnostic {
    let mut removed_table = Table::new();
    removed_table.set_title("Removed sections:");
    removed_table.add_row(["Section name", "Source"]);
    for section in removed {
        removed_table.add_row([section_name(object, section.id), section.source.to_string()]);
    }

    Diagnostic::new(DiagnosticKind::DebugPrint, "garbage collector outcome").add(removed_table)
}
