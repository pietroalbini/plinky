mod table;

pub use self::table::Table;

pub trait Widget {
    fn render(&self, writer: &mut dyn WidgetWriter);

    fn render_to_string(&self) -> String {
        let mut buffer = String::new();
        self.render(&mut buffer);
        buffer
    }
}

pub trait WidgetWriter {
    fn push(&mut self, content: char);

    fn push_str(&mut self, content: &str) {
        for chr in content.chars() {
            self.push(chr);
        }
    }
}

impl WidgetWriter for String {
    fn push(&mut self, content: char) {
        self.push(content);
    }

    fn push_str(&mut self, content: &str) {
        self.push_str(content);
    }
}
