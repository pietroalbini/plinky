use crate::widgets::Widget;
use crate::WidgetWriter;
use crate::writer::IndentMode;

pub struct WidgetGroup {
    widgets: Vec<Box<dyn Widget>>,
}

impl WidgetGroup {
    pub fn new() -> Self {
        WidgetGroup { widgets: Vec::new() }
    }

    pub fn add<T: Widget + 'static>(mut self, widget: T) -> Self {
        self.widgets.push(Box::new(widget));
        self
    }
}

impl Widget for WidgetGroup {
    fn render(&self, writer: &mut WidgetWriter) {
        writer.push_indent(" │", IndentMode::ShowAlways);
        writer.push_indent("  ", IndentMode::HideOnEmptyLines);
        for (idx, widget) in self.widgets.iter().enumerate() {
            if idx == 0 {
                writer.push_str("\n");
            } else {
                writer.push_str("\n\n");
            }
            widget.render(writer);
            if writer.last_char() == Some('\n') {
                panic!("widgets must not terminate with a newline");
            }
        }
        writer.pop_indent();
        writer.pop_indent();
        if !self.widgets.is_empty() {
            writer.push_str("\n ┴");
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
    fn test_empty_group() {
        assert!(WidgetGroup::new().render_to_string().is_empty());
    }

    #[test]
    fn test_multiple_widgets() {
        let _config = configure_insta();

        let mut table = Table::new();
        table.add_row(["Foo", "Bar"]);

        let group = WidgetGroup::new().add(Text::new("A simple text message!")).add(table);
        assert_snapshot!(group.render_to_string());
    }
}
