use crate::diagnostics::builders::UndefinedSymbolDiagnostic;
use crate::repr::object::Object;
use crate::repr::symbols::views::DefinedSymbols;
use plinky_diagnostics::GatheredContext;
use plinky_diagnostics::widgets::{Table, Text, Widget};
use plinky_utils::jaro_similarity;

pub(super) fn generate(
    diagnostic: &UndefinedSymbolDiagnostic,
    ctx: &GatheredContext<'_>,
) -> Vec<Box<dyn Widget>> {
    let object: &Object = ctx.required();

    let undefined_name = diagnostic.name.resolve();
    let mut candidates = Vec::new();
    for symbol in object.symbols.iter(&DefinedSymbols) {
        if symbol.visibility() != diagnostic.expected_visibility {
            continue;
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
