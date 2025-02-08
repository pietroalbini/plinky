use plinky_diagnostics::widgets::{QuotedText, Text};
use plinky_diagnostics::{Diagnostic, DiagnosticBuilder, DiagnosticKind, GatheredContext};
use std::path::{PathBuf};

#[derive(Debug)]
pub(crate) struct NoSymbolNameAtArchiveStartDiagnostic {
    pub(crate) archive_path: PathBuf,
}

impl DiagnosticBuilder for NoSymbolNameAtArchiveStartDiagnostic {
    fn build(&self, _ctx: &GatheredContext<'_>) -> Diagnostic {
        Diagnostic::new(
            DiagnosticKind::Error,
            "the first member of the archive is not a symbol table",
        )
        .add(Text::new(format!("file: {}", self.archive_path.display())))
        .add(Text::new(
            "help: you can pass the `-s` flag to `ar` when building the archive, \
                or add the table to an existing archive by running:",
        ))
        .add(QuotedText::new(format!("ranlib {}", self.archive_path.display())))
    }
}
