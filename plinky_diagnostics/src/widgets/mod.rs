mod group;
mod hex_dump;
mod quoted_text;
mod table;
mod text;

pub use self::group::WidgetGroup;
pub use self::quoted_text::QuotedText;
pub use self::table::Table;
pub use self::text::Text;
pub use self::hex_dump::HexDump;
use crate::WidgetWriter;

pub trait Widget {
    fn render(&self, writer: &mut WidgetWriter<'_>);

    fn render_to_string(&self) -> String {
        let mut buffer = String::new();
        let mut writer = WidgetWriter::new(&mut buffer);
        self.render(&mut writer);
        buffer
    }
}
