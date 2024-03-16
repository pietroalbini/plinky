use crate::widgets::Widget;
use crate::WidgetWriter;

pub struct Text {
    content: String,
}

impl Text {
    pub fn new(content: impl Into<String>) -> Self {
        Self { content: content.into() }
    }
}

impl Widget for Text {
    fn render(&self, writer: &mut WidgetWriter) {
        writer.push_str(&self.content);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_text() {
        assert_eq!("Hello world!", Text::new("Hello world!").render_to_string());
    }
}
