use crate::debug_print::names::Names;
use crate::passes::analyze_relocations::{PlannedGot, RelocsAnalysis, ResolvedAt};
use crate::repr::object::Object;
use plinky_diagnostics::widgets::{Table, Text, Widget, WidgetGroup};
use plinky_diagnostics::{Diagnostic, DiagnosticKind};

pub(super) fn render_relocs_analysis(object: &Object, analysis: &RelocsAnalysis) -> Diagnostic {
    let names = Names::new(object);

    Diagnostic::new(DiagnosticKind::DebugPrint, "relocations analysis")
        .add_iter(analysis.got.as_ref().map(|got| render_got(".got.plt", &names, got)))
        .add_iter(analysis.got_plt.as_ref().map(|got| render_got(".got.plt", &names, got)))
}

fn render_got(name: &str, names: &Names, got: &PlannedGot) -> Box<dyn Widget> {
    let symbols: Box<dyn Widget> = if got.symbols().next().is_some() {
        let mut symbols = Table::new();
        symbols.set_title("Symbols:");
        symbols.add_row(["Name", "Resolved at"]);
        for symbol in got.symbols() {
            symbols.add_row([
                names.symbol(symbol.id),
                match symbol.resolved_at {
                    ResolvedAt::LinkTime => "link time",
                    ResolvedAt::RunTime => "runtime",
                },
            ]);
        }
        Box::new(symbols)
    } else {
        Box::new(Text::new("no symbols within this GOT"))
    };

    Box::new(WidgetGroup::new().name(format!("global offset table {name}")).add(symbols))
}
