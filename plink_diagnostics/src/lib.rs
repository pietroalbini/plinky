mod table;
mod span;
mod diagnostic;

pub use crate::table::Table;
pub use crate::span::ObjectSpan;
pub use crate::diagnostic::{Diagnostic, DiagnosticKind};

#[cfg(test)]
#[must_use]
fn configure_insta() -> impl Drop {
    use insta::Settings;

    let mut settings = Settings::clone_current();
    settings.set_snapshot_path(concat!(env!("CARGO_MANIFEST_DIR"), "/snapshots"));

    settings.bind_to_scope()
}
