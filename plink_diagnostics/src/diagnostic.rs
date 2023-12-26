use crate::widgets::{Widget, WidgetGroup};

pub struct Diagnostic {
    kind: DiagnosticKind,
    message: String,
    children: WidgetGroup,
}

impl Diagnostic {
    pub fn new(kind: DiagnosticKind, message: impl Into<String>) -> Self {
        Self { kind, message: message.into(), children: WidgetGroup::new() }
    }

    pub fn add<T: Widget + 'static>(mut self, widget: T) -> Self {
        self.children = self.children.add(widget);
        self
    }
}

impl std::fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.kind, self.message)?;

        let children = self.children.render_to_string();
        if !children.is_empty() {
            write!(f, "\n{children}")?;
        }

        Ok(())
    }
}

pub enum DiagnosticKind {
    Error,
    Warning,
    DebugPrint,
}

impl std::fmt::Display for DiagnosticKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DiagnosticKind::Error => f.write_str("error"),
            DiagnosticKind::Warning => f.write_str("warning"),
            DiagnosticKind::DebugPrint => f.write_str("debug print"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::configure_insta;
    use crate::widgets::{Table, Text};
    use insta::assert_snapshot;

    #[test]
    fn test_kind_debug_print() {
        let _config = configure_insta();

        let diagnostic = Diagnostic::new(DiagnosticKind::DebugPrint, "this is a debug print");
        assert_snapshot!(diagnostic.to_string());
    }

    #[test]
    fn test_kind_error() {
        let _config = configure_insta();

        let diagnostic = Diagnostic::new(DiagnosticKind::Error, "something went wrong");
        assert_snapshot!(diagnostic.to_string());
    }

    #[test]
    fn test_kind_warning() {
        let _config = configure_insta();

        let diagnostic = Diagnostic::new(DiagnosticKind::Warning, "something bad might happen");
        assert_snapshot!(diagnostic.to_string());
    }

    #[test]
    fn test_with_children() {
        let _config = configure_insta();

        let mut table = Table::new();
        table.add_row(["Foo", "Bar"]);

        let diagnostic = Diagnostic::new(DiagnosticKind::Error, "something went wrong")
            .add(Text::new("you can learn more from this table:"))
            .add(table);
        assert_snapshot!(diagnostic.to_string());
    }
}
