use crate::interner::Interned;
use plinky_diagnostics::{Diagnostic, DiagnosticBuilder, GatheredContext};

#[derive(Debug)]
pub(crate) struct UndefinedSymbolDiagnostic {
    pub(crate) name: Interned<String>,
}

impl DiagnosticBuilder for UndefinedSymbolDiagnostic {
    fn build(&self, _ctx: &GatheredContext<'_>) -> Diagnostic {
        Diagnostic::new(
            plinky_diagnostics::DiagnosticKind::Error,
            format!("undefined symbol: {}", self.name),
        )
    }
}
