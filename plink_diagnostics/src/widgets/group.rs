use super::WidgetWriter;
use crate::widgets::Widget;

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
    fn render(&self, writer: &mut dyn WidgetWriter) {
        let mut indent_writer = IndentWriter {
            initial: true,
            last_char_is_newline: false,
            pattern: " │ ",
            inner: writer,
        };
        for (idx, widget) in self.widgets.iter().enumerate() {
            if idx == 0 {
                indent_writer.push_str("\n");
            } else {
                indent_writer.push_str("\n\n");
            }
            widget.render(&mut indent_writer);
            if indent_writer.last_char_is_newline {
                panic!("widgets must not terminate with a newline");
            }
        }
        if !self.widgets.is_empty() {
            writer.push_str("\n ┴");
        }
    }
}

struct IndentWriter<'a> {
    initial: bool,
    last_char_is_newline: bool,
    pattern: &'a str,
    inner: &'a mut dyn WidgetWriter,
}

impl WidgetWriter for IndentWriter<'_> {
    fn push(&mut self, content: char) {
        if self.initial {
            self.inner.push_str(self.pattern);
            self.initial = false;
        }
        self.inner.push(content);
        if content == '\n' {
            self.inner.push_str(self.pattern);
            self.last_char_is_newline = true;
        } else {
            self.last_char_is_newline = false;
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
