use plinky_diagnostics::widgets::{QuotedText, Text};
use plinky_diagnostics::{Diagnostic, DiagnosticKind};
use std::path::Path;

pub(crate) fn build(archive_path: &Path) -> Diagnostic {
    Diagnostic::new(DiagnosticKind::Error, "the first member of the archive is not a symbol table")
        .add(Text::new(format!("file: {}", archive_path.display())))
        .add(Text::new(
            "help: you can pass the `-s` flag to `ar` when building the archive, \
                or add the table to an existing archive by running:",
        ))
        .add(QuotedText::new(format!("ranlib {}", archive_path.display())))
}
