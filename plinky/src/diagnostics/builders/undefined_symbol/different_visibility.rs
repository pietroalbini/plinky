use crate::diagnostics::builders::UndefinedSymbolDiagnostic;
use crate::repr::object::Object;
use crate::repr::symbols::SymbolVisibility;
use crate::repr::symbols::views::DefinedSymbols;
use plinky_diagnostics::GatheredContext;
use plinky_diagnostics::widgets::{Text, Widget};

pub(super) fn generate(
    diagnostic: &UndefinedSymbolDiagnostic,
    ctx: &GatheredContext<'_>,
) -> Vec<Box<dyn Widget>> {
    let object: &Object = ctx.required();

    for symbol in object.symbols.iter(&DefinedSymbols) {
        if symbol.name() != diagnostic.name {
            continue;
        }

        // We found the same symbol we were actually looking for, why was this diagnostic emitted?
        if symbol.visibility() == diagnostic.expected_visibility {
            panic!("bug: missing symbol found in diagnostic code");
        }

        return vec![
            Box::new(Text::new(format!(
                "help: a symbol with the same name exists, but it's {}",
                pretty_visibility(&symbol.visibility())
            ))),
            Box::new(Text::new(format!(
                "note: the linker is looking for a {} symbol",
                pretty_visibility(&diagnostic.expected_visibility)
            ))),
        ];
    }

    Vec::new()
}

fn pretty_visibility(visibility: &SymbolVisibility) -> &'static str {
    match visibility {
        SymbolVisibility::Local => "local",
        SymbolVisibility::Global { weak: false, hidden: false } => "global",
        SymbolVisibility::Global { weak: true, hidden: false } => "weak",
        SymbolVisibility::Global { weak: false, hidden: true } => "global hidden",
        SymbolVisibility::Global { weak: true, hidden: true } => "weak hidden",
    }
}
