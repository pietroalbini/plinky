use crate::diagnostics::contexts::WhileProcessingEntrypoint;
use crate::interner::Interned;
use plinky_diagnostics::widgets::{Text, Widget};
use plinky_diagnostics::{Diagnostic, DiagnosticBuilder, DiagnosticKind, GatheredContext};

#[derive(Debug)]
pub(crate) struct UndefinedSymbolDiagnostic {
    pub(crate) name: Interned<String>,
}

impl DiagnosticBuilder for UndefinedSymbolDiagnostic {
    fn build(&self, ctx: &GatheredContext<'_>) -> Diagnostic {
        Diagnostic::new(DiagnosticKind::Error, format!("undefined symbol: {}", self.name))
            .add_iter(self.entry_point_note(ctx))
    }
}

impl UndefinedSymbolDiagnostic {
    fn entry_point_note(&self, ctx: &GatheredContext<'_>) -> Vec<Box<dyn Widget>> {
        let mut widgets: Vec<Box<dyn Widget>> = Vec::new();

        if ctx.has::<WhileProcessingEntrypoint>() {
            widgets.push(Box::new(Text::new(format!(
                "note: `{}` is the entry point of the executable",
                self.name
            ))));
        }

        widgets
    }
}
