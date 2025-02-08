use crate::cli::{CliOptions, EntryPoint};
use crate::diagnostics::contexts::WhileProcessingEntrypoint;
use crate::interner::Interned;
use crate::repr::object::Object;
use crate::repr::symbols::views::AllSymbols;
use crate::repr::symbols::SymbolValue;
use plinky_diagnostics::widgets::{Table, Text, Widget};
use plinky_diagnostics::{Diagnostic, DiagnosticBuilder, DiagnosticKind, GatheredContext};
use plinky_utils::jaro_similarity;

#[derive(Debug)]
pub(crate) struct UndefinedSymbolDiagnostic {
    pub(crate) name: Interned<String>,
}

impl DiagnosticBuilder for UndefinedSymbolDiagnostic {
    fn build(&self, ctx: &GatheredContext<'_>) -> Diagnostic {
        Diagnostic::new(DiagnosticKind::Error, format!("undefined symbol: {}", self.name))
            .add_iter(self.similar_symbols(ctx))
            .add_iter(self.entry_point_note(ctx))
    }
}

impl UndefinedSymbolDiagnostic {
    fn similar_symbols(&self, ctx: &GatheredContext<'_>) -> Vec<Box<dyn Widget>> {
        let object: &Object = ctx.required();

        let undefined_name = self.name.resolve();
        let mut candidates = Vec::new();
        for symbol in object.symbols.iter(&AllSymbols) {
            match symbol.value() {
                SymbolValue::Absolute { .. }
                | SymbolValue::Section { .. }
                | SymbolValue::SectionRelative { .. }
                | SymbolValue::SectionVirtualAddress { .. }
                | SymbolValue::ExternallyDefined => {}
                SymbolValue::SectionNotLoaded | SymbolValue::Undefined | SymbolValue::Null => {
                    continue
                }
            }

            let symbol_name = symbol.name().resolve();
            let similarity = jaro_similarity(&undefined_name, &symbol_name);
            if similarity > 0.7 {
                candidates.push((symbol_name, similarity));
            }
        }

        candidates.sort_by(|(_, lhs_similarity), (_, rhs_similarity)| {
            lhs_similarity.partial_cmp(rhs_similarity).unwrap().reverse()
        });

        let mut table = Table::new();
        for (candidate, _similarity) in candidates.iter().take(3) {
            table.add_body([candidate.as_str()]);
        }

        if table.is_body_empty() {
            Vec::new()
        } else {
            vec![
                Box::new(Text::new("help: the following symbols with a similar name exist:")),
                Box::new(table),
            ]
        }
    }

    fn entry_point_note(&self, ctx: &GatheredContext<'_>) -> Vec<Box<dyn Widget>> {
        let mut widgets: Vec<Box<dyn Widget>> = Vec::new();

        if ctx.has::<WhileProcessingEntrypoint>() {
            let cli: &CliOptions = ctx.required();

            widgets.push(Box::new(Text::new(format!(
                "note: `{}` is the entry point of the executable",
                self.name
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
}
