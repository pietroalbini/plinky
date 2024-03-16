pub struct WidgetWriter<'a> {
    buffer: &'a mut String,

    indent: Vec<Indent>,
    last_char: Option<char>,
}

impl<'a> WidgetWriter<'a> {
    pub fn new(buffer: &'a mut String) -> Self {
        Self { buffer, indent: Vec::new(), last_char: None }
    }

    pub(crate) fn push(&mut self, content: char) {
        let indents_to_consider = if content == '\n' {
            indents_for_empty_line(&mut self.indent)
        } else {
            &mut self.indent
        };
        for indent in indents_to_consider {
            if !indent.printed_this_line {
                self.buffer.push_str(&indent.content);
                indent.printed_this_line = true;
            }
        }

        self.buffer.push(content);
        self.last_char = Some(content);

        if content == '\n' {
            for indent in &mut self.indent {
                indent.printed_this_line = false;
            }
        }
    }

    pub(crate) fn push_str(&mut self, content: &str) {
        for chr in content.chars() {
            self.push(chr);
        }
    }

    pub(crate) fn push_indent(&mut self, indent: &str, mode: IndentMode) {
        self.indent.push(Indent::new(indent, mode));
    }

    pub(crate) fn pop_indent(&mut self) {
        self.indent.pop();
    }

    pub(crate) fn last_char(&self) -> Option<char> {
        self.last_char
    }
}

fn indents_for_empty_line(indents: &mut [Indent]) -> &mut [Indent] {
    let skip = indents
        .iter()
        .rev()
        .take_while(|indent| matches!(indent.mode, IndentMode::HideOnEmptyLines))
        .count();
    let len = indents.len();
    &mut indents[..len - skip]
}

#[derive(Debug, PartialEq, Eq)]
struct Indent {
    content: String,
    mode: IndentMode,
    printed_this_line: bool,
}

impl Indent {
    fn new(content: &str, mode: IndentMode) -> Self {
        Self { content: content.to_string(), mode, printed_this_line: false }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub(crate) enum IndentMode {
    ShowAlways,
    HideOnEmptyLines,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_indents_for_empty_line() {
        let i = |mode| Indent::new("_", mode);

        assert_eq!(&mut [] as &mut [Indent], indents_for_empty_line(&mut []));
        assert_eq!(
            &mut [] as &mut [Indent],
            indents_for_empty_line(&mut [i(IndentMode::HideOnEmptyLines)])
        );
        assert_eq!(
            &mut [i(IndentMode::ShowAlways)],
            indents_for_empty_line(&mut [i(IndentMode::ShowAlways)])
        );
        assert_eq!(
            &mut [i(IndentMode::ShowAlways)],
            indents_for_empty_line(&mut [
                i(IndentMode::ShowAlways),
                i(IndentMode::HideOnEmptyLines)
            ])
        );
        assert_eq!(
            &mut [
                i(IndentMode::ShowAlways),
                i(IndentMode::HideOnEmptyLines),
                i(IndentMode::HideOnEmptyLines),
                i(IndentMode::ShowAlways),
            ],
            indents_for_empty_line(&mut [
                i(IndentMode::ShowAlways),
                i(IndentMode::HideOnEmptyLines),
                i(IndentMode::HideOnEmptyLines),
                i(IndentMode::ShowAlways),
            ])
        );
        assert_eq!(
            &mut [
                i(IndentMode::ShowAlways),
                i(IndentMode::HideOnEmptyLines),
                i(IndentMode::HideOnEmptyLines),
                i(IndentMode::ShowAlways),
            ],
            indents_for_empty_line(&mut [
                i(IndentMode::ShowAlways),
                i(IndentMode::HideOnEmptyLines),
                i(IndentMode::HideOnEmptyLines),
                i(IndentMode::ShowAlways),
                i(IndentMode::HideOnEmptyLines),
                i(IndentMode::HideOnEmptyLines),
            ])
        );
    }
}
