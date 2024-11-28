use crate::WidgetWriter;
use crate::widgets::{Table, Widget};
use std::fmt::Write;

const BYTES_PER_LINE: usize = 16;

pub struct HexDump {
    data: Vec<u8>,
}

impl HexDump {
    pub fn new(data: impl Into<Vec<u8>>) -> Self {
        Self { data: data.into() }
    }
}

impl Widget for HexDump {
    fn render(&self, writer: &mut WidgetWriter<'_>) {
        let mut hex = String::new();
        let mut ascii = String::new();

        for chunk in self.data.chunks(BYTES_PER_LINE) {
            for (i, byte) in chunk.iter().copied().enumerate() {
                if i > 0 {
                    hex.push(' ');
                }
                hex.write_fmt(format_args!("{byte:0>2x}")).unwrap();
                if show_as_ascii(byte) {
                    ascii.push(byte as char);
                } else {
                    ascii.push('.');
                }
            }
            hex.push('\n');
            ascii.push('\n');
        }

        let mut table = Table::new();
        table.add_row([hex.trim(), ascii.trim()]);
        table.render(writer);
    }
}

fn show_as_ascii(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte.is_ascii_punctuation() || byte == b' '
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::configure_insta;
    use insta::assert_snapshot;

    #[test]
    fn test_hex_dump_hello() {
        let _config = configure_insta();

        assert_snapshot!(HexDump::new(b"Hello world").render_to_string());
    }

    #[test]
    fn test_hex_dump_256() {
        let _config = configure_insta();

        let data = (0u8..=255u8).collect::<Vec<_>>();
        assert_snapshot!(HexDump::new(data).render_to_string());
    }
}
