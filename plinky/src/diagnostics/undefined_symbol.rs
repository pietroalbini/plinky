use crate::diagnostics::WhileProcessingEntrypoint;
use crate::interner::Interned;
use plinky_diagnostics::widgets::Text;
use plinky_diagnostics::{Diagnostic, DiagnosticBuilder, DiagnosticKind, GatheredContext};

#[derive(Debug)]
pub(crate) struct UndefinedSymbolDiagnostic {
    pub(crate) name: Interned<String>,
}

impl DiagnosticBuilder for UndefinedSymbolDiagnostic {
    fn build(&self, ctx: &GatheredContext<'_>) -> Diagnostic {
        let mut diagnostic =
            Diagnostic::new(DiagnosticKind::Error, format!("undefined symbol: {}", self.name));

        if ctx.has::<WhileProcessingEntrypoint>() {
            diagnostic = diagnostic.add(Text::new(format!(
                "note: `{}` is the entry point of the executable",
                self.name
            )));
        }

        diagnostic
    }
}
