use crate::interner::Interned;
use plinky_diagnostics::Diagnostic;

pub(crate) fn build(name: Interned<String>) -> Diagnostic {
    Diagnostic::new(plinky_diagnostics::DiagnosticKind::Error, format!("undefined symbol: {name}"))
}
