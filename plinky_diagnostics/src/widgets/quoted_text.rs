use crate::WidgetWriter;
use crate::widgets::Widget;
use crate::writer::IndentMode;

pub struct QuotedText {
    content: String,
}

impl QuotedText {
    pub fn new(content: impl Into<String>) -> Self {
        Self { content: content.into() }
    }
}

impl Widget for QuotedText {
    fn render(&self, writer: &mut WidgetWriter) {
        if self.content.is_empty() {
            return;
        }

        writer.push_str("╭\n");

        writer.push_indent("│", IndentMode::ShowAlways);
        writer.push_indent(" ", IndentMode::HideOnEmptyLines);

        writer.push_str(&self.content);
        writer.push('\n');

        writer.pop_indent();
        writer.pop_indent();

        writer.push_str("╰");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::configure_insta;
    use insta::assert_snapshot;

    #[test]
    fn test_empty_content() {
        assert_eq!("", QuotedText::new("").render_to_string());
    }

    #[test]
    fn test_single_line() {
        let _config = configure_insta();

        let content = QuotedText::new("Hello world");
        assert_snapshot!(content.render_to_string());
    }

    #[test]
    fn test_multiple_lines() {
        let _config = configure_insta();

        let content = QuotedText::new("Hello world\nThis\n\n  has\n    multiple lines!");
        assert_snapshot!(content.render_to_string());
    }
}
