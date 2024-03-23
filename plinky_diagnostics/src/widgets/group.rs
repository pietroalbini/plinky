use crate::widgets::Widget;
use crate::writer::IndentMode;
use crate::WidgetWriter;

pub struct WidgetGroup {
    name: Option<String>,
    widgets: Vec<Box<dyn Widget>>,
}

impl WidgetGroup {
    pub fn new() -> Self {
        WidgetGroup { name: None, widgets: Vec::new() }
    }

    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn add<T: Widget + 'static>(mut self, widget: T) -> Self {
        self.widgets.push(Box::new(widget));
        self
    }

    pub fn add_iter<T, I>(mut self, iter: I) -> Self
    where
        T: Widget + 'static,
        I: IntoIterator<Item = T>,
    {
        for widget in iter {
            self.widgets.push(Box::new(widget));
        }
        self
    }
}

impl Widget for WidgetGroup {
    fn render(&self, writer: &mut WidgetWriter) {
        if let Some(name) = &self.name {
            writer.push_str(name);
            writer.push('\n');
        }
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

    #[test]
    fn test_with_name() {
        let _config = configure_insta();

        let group = WidgetGroup::new().name("example name").add(Text::new("A simple text message!"));
        assert_snapshot!(group.render_to_string());
    }
}
