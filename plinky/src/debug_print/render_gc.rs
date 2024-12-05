use crate::debug_print::names::Names;
use crate::passes::gc_sections::RemovedSection;
use crate::repr::object::Object;
use plinky_diagnostics::widgets::Table;
use plinky_diagnostics::{Diagnostic, DiagnosticKind};

pub(super) fn render_gc(object: &Object, removed: &[RemovedSection]) -> Diagnostic {
    let names = Names::new(object);

    let mut removed_table = Table::new();
    removed_table.set_title("Removed sections:");
    removed_table.add_head(["Section name", "Source"]);
    for section in removed {
        removed_table.add_body([names.section(section.id), &section.source.to_string()]);
    }

    Diagnostic::new(DiagnosticKind::DebugPrint, "garbage collector outcome").add(removed_table)
}
