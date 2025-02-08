mod builder;
mod diagnostic;
mod span;
pub mod widgets;
mod writer;

pub use crate::builder::{DiagnosticBuilder, GatheredContext};
pub use crate::diagnostic::{Diagnostic, DiagnosticKind};
pub use crate::span::ObjectSpan;
pub use crate::writer::WidgetWriter;

#[cfg(test)]
#[must_use]
fn configure_insta() -> impl Drop {
    use insta::Settings;

    let mut settings = Settings::clone_current();
    settings.set_snapshot_path(concat!(env!("CARGO_MANIFEST_DIR"), "/snapshots"));

    settings.bind_to_scope()
}
