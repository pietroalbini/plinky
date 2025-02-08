use crate::interner::Interned;
use plinky_diagnostics::{Diagnostic, DiagnosticBuilder, DiagnosticKind, GatheredContext};

#[derive(Debug)]
pub(crate) struct UndefinedSymbolDiagnostic {
    pub(crate) name: Interned<String>,
}

impl DiagnosticBuilder for UndefinedSymbolDiagnostic {
    fn build(&self, _ctx: &GatheredContext<'_>) -> Diagnostic {
        Diagnostic::new(DiagnosticKind::Error, format!("undefined symbol: {}", self.name))
    }
}
