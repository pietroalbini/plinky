use plinky_diagnostics::DiagnosticContext;

pub(crate) mod no_symbol_table_at_archive_start;
pub(crate) mod undefined_symbol;

#[derive(Debug)]
pub(crate) struct WhileProcessingEntrypoint;

impl DiagnosticContext for WhileProcessingEntrypoint {}
