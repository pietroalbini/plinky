mod entry_point_note;
mod similar_symbols;

use crate::interner::Interned;
use plinky_diagnostics::{Diagnostic, DiagnosticBuilder, DiagnosticKind, GatheredContext};

#[derive(Debug)]
pub(crate) struct UndefinedSymbolDiagnostic {
    pub(crate) name: Interned<String>,
}

impl DiagnosticBuilder for UndefinedSymbolDiagnostic {
    fn build(&self, ctx: &GatheredContext<'_>) -> Diagnostic {
        Diagnostic::new(DiagnosticKind::Error, format!("undefined symbol: {}", self.name))
            .add_iter(similar_symbols::generate(self, ctx))
            .add_iter(entry_point_note::generate(self, ctx))
    }
}
