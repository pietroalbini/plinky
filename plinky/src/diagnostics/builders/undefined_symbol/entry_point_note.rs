use crate::diagnostics::builders::UndefinedSymbolDiagnostic;
use crate::diagnostics::contexts::WhileProcessingEntrypoint;
use plinky_diagnostics::widgets::{Text, Widget};
use plinky_diagnostics::GatheredContext;
use crate::cli::{CliOptions, EntryPoint};

pub(super) fn generate(
    diagnostic: &UndefinedSymbolDiagnostic,
    ctx: &GatheredContext<'_>,
) -> Vec<Box<dyn Widget>> {
    let mut widgets: Vec<Box<dyn Widget>> = Vec::new();

    if ctx.has::<WhileProcessingEntrypoint>() {
        let cli: &CliOptions = ctx.required();

        widgets.push(Box::new(Text::new(format!(
            "note: `{}` is the entry point of the executable",
            diagnostic.name
        ))));

        let message = match &cli.entry {
            EntryPoint::None => unreachable!(),
            EntryPoint::Default => {
                "this is the default entry point for the platform, \
                     pass `-e <name> to customize it"
            }
            EntryPoint::Custom(_) => "the entry point was customized with the `-e` flag",
        };
        widgets.push(Box::new(Text::new(format!("note: {message}"))));
    }

    widgets
}
