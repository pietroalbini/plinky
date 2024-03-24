use crate::widgets::Widget;
use crate::WidgetWriter;

const UNICODE_CHARSET: TableCharset = TableCharset {
    vertical_separator: '│',
    horizontal_separator: '─',
    first_junction: TableJunctionCharset { first: '╭', middle: '┬', last: '╮' },
    middle_junction: TableJunctionCharset { first: '├', middle: '┼', last: '┤' },
    last_junction: TableJunctionCharset { first: '╰', middle: '┴', last: '╯' },
};

pub struct Table {
    charset: &'static TableCharset,
    title: Option<String>,
    state: TableState,
}

impl Table {
    pub fn new() -> Self {
        Self { charset: &UNICODE_CHARSET, title: None, state: TableState::Empty }
    }

    pub fn set_title(&mut self, title: impl Into<String>) {
        self.title = Some(title.into());
    }

    pub fn add_row<I, V>(&mut self, row: I)
    where
        V: Into<String>,
        I: IntoIterator<Item = V>,
    {
        let mut count = 0;
        let row = row
            .into_iter()
            .map(|cell| cell.into().split('\n').map(|s| s.to_string()).collect::<Vec<_>>())
            .inspect(|_| count += 1);

        match &mut self.state {
            TableState::Empty => {
                let mut cells_len = Vec::new();
                self.state = TableState::HasContent {
                    content: row
                        .inspect(|cell| {
                            cells_len.push(cell.iter().map(|l| l.len()).max().unwrap_or(0))
                        })
                        .collect(),
                    cells_len,
                    cells_count: count,
                };
            }
            TableState::HasContent { cells_count, cells_len, content } => {
                content.extend(row.enumerate().map(|(pos, cell)| {
                    let cell_len = cell.iter().map(|c| c.len()).max().unwrap_or(0);
                    match cells_len.get_mut(pos) {
                        Some(len) => *len = (*len).max(cell_len),
                        None => cells_len.push(cell_len),
                    }
                    cell
                }));
                if count != *cells_count {
                    panic!("other rows have {cells_count} cells, while this row has {count} cells");
                }
            }
        }
    }

    fn render_horizontal_border(
        &self,
        writer: &mut WidgetWriter,
        cells_len: &[usize],
        junction: &TableJunctionCharset,
    ) {
        writer.push(junction.first);
        for (idx, len) in cells_len.iter().enumerate() {
            if idx != 0 {
                writer.push(junction.middle);
            }
            for _ in 0..(*len + 2) {
                writer.push(self.charset.horizontal_separator);
            }
        }
        writer.push(junction.last);
    }
}

impl Widget for Table {
    fn render(&self, writer: &mut WidgetWriter) {
        let TableState::HasContent { cells_count, cells_len, content } = &self.state else {
            panic!("trying to render an empty table");
        };

        if let Some(title) = &self.title {
            writer.push_str("  ");
            writer.push_str(title);
            writer.push_str("\n");
        }
        self.render_horizontal_border(writer, cells_len, &self.charset.first_junction);
        writer.push_str("\n");

        let mut rows = content.chunks_exact(*cells_count);
        let mut idx = 0;
        for row in rows.by_ref() {
            let last_row = idx == content.len() / cells_count - 1;
            let lines_count = row.iter().map(|cell| cell.len()).max().unwrap_or(0);

            for line in 0..lines_count {
                writer.push(self.charset.vertical_separator);
                for (idx, cell_all_lines) in row.iter().enumerate() {
                    let cell = cell_all_lines.get(line).map(|c| c.as_str()).unwrap_or_default();
                    writer.push(' ');
                    writer.push_str(cell);

                    // Padding to align all cells.
                    for _ in cell.len()..cells_len[idx] {
                        writer.push(' ');
                    }

                    writer.push(' ');
                    writer.push(self.charset.vertical_separator);
                }
                writer.push('\n');
            }
            self.render_horizontal_border(
                writer,
                cells_len,
                if last_row { &self.charset.last_junction } else { &self.charset.middle_junction },
            );
            if !last_row {
                writer.push('\n');
            }

            idx += 1;
        }

        assert!(rows.remainder().is_empty());
    }
}

enum TableState {
    Empty,
    HasContent { cells_count: usize, cells_len: Vec<usize>, content: Vec<Vec<String>> },
}

struct TableCharset {
    vertical_separator: char,
    horizontal_separator: char,
    first_junction: TableJunctionCharset,
    middle_junction: TableJunctionCharset,
    last_junction: TableJunctionCharset,
}

struct TableJunctionCharset {
    first: char,
    middle: char,
    last: char,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::configure_insta;
    use insta::assert_snapshot;

    #[test]
    fn test_sample_table() {
        let _config = configure_insta();

        let mut table = Table::new();
        table.add_row(["Foo", "Bar", "Baz"]);
        table.add_row(["Hello", "super long", "world!"]);
        table.add_row(["98%", "", "-"]);

        assert_snapshot!(table.render_to_string());
    }

    #[test]
    fn test_single_cell_table() {
        let _config = configure_insta();

        let mut table = Table::new();
        table.add_row(["alone"]);

        assert_snapshot!(table.render_to_string());
    }

    #[test]
    fn test_table_with_title() {
        let _config = configure_insta();

        let mut table = Table::new();
        table.set_title("Example title:");
        table.add_row(["a", "b", "c"]);

        assert_snapshot!(table.render_to_string());
    }

    #[test]
    fn test_table_with_multiple_lines() {
        let _config = configure_insta();

        let mut table = Table::new();
        table.add_row(["a", "b", "c"]);
        table.add_row(["foo\nbar", "baz", "qu\nu\n\n\nx!!!!!!!!"]);

        assert_snapshot!(table.render_to_string());
    }

    #[test]
    #[should_panic = "other rows have 3 cells, while this row has 2 cells"]
    fn test_wrong_number_of_cells() {
        let mut table = Table::new();
        table.add_row(["foo", "bar", "baz"]);
        table.add_row(["a", "b"]);
    }

    #[test]
    #[should_panic = "trying to render an empty table"]
    fn test_empty_table() {
        Table::new().render_to_string();
    }
}
