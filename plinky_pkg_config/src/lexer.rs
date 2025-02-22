use plinky_macros::{Display, Error};
use std::fmt::Display;
use std::iter::Peekable;
use std::str::Chars;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Token {
    Text(String),
    Variable(String),
    Whitespace(String),
    Equals,
    Colon,
    NewLine,
}

impl Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Token::Text(text) => write!(f, "{text:?}"),
            Token::Variable(var) => write!(f, "${{{var}}}"),
            Token::Whitespace(_) => f.write_str("whitespace"),
            Token::Equals => f.write_str("="),
            Token::Colon => f.write_str(":"),
            Token::NewLine => f.write_str("newline"),
        }
    }
}

pub(crate) struct Lexer<'a> {
    input: Peekable<Chars<'a>>,
    result: Vec<Token>,
    text_buffer: String,
}

impl<'a> Lexer<'a> {
    pub(crate) fn new(input: &'a str) -> Self {
        Self { input: input.chars().peekable(), result: Vec::new(), text_buffer: String::new() }
    }

    pub(crate) fn lex(mut self) -> Result<Vec<Token>, LexError> {
        while let Some(c) = self.input.next() {
            if c == '\\' {
                match self.input.peek().copied() {
                    // \# gets turned into a verbatim #, escaping the comment.
                    Some('#') => {
                        let _ = self.input.next();
                        self.text_buffer.push('#');
                    }
                    // An escaped newline disappears, concatenating the two lines.
                    Some(c @ '\n') | Some(c @ '\r') => {
                        let _ = self.input.next();
                        self.maybe_consume_second_newline(c);
                    }
                    // In any other case, include the \ verbatim.
                    Some(_) | None => self.text_buffer.push('\\'),
                }
            } else if c == '=' {
                self.flush_text();
                self.result.push(Token::Equals);
            } else if c == ':' {
                self.flush_text();
                self.result.push(Token::Colon);
            } else if c == '\n' || c == '\r' {
                self.flush_text();
                self.maybe_consume_second_newline(c);
                self.result.push(Token::NewLine);
            } else if c == '$' {
                match self.input.next() {
                    Some('{') => {
                        self.flush_text();
                        let mut name = String::new();
                        loop {
                            match self.input.next() {
                                Some('}') => {
                                    self.result.push(Token::Variable(name));
                                    break;
                                }
                                Some(c) if is_valid_identifier(c) => name.push(c),
                                Some(c) => return Err(LexError::InvalidVariableChar(c)),
                                None => return Err(LexError::MissingCloseVariable),
                            }
                        }
                    }
                    Some('$') => {
                        // For whatever reason, $ is escaped as $$.
                        self.text_buffer.push('$');
                    }
                    Some(other) => {
                        self.text_buffer.push(c);
                        self.text_buffer.push(other);
                    }
                    None => self.text_buffer.push(c),
                }
            } else if c == '#' {
                // Skip comments.
                loop {
                    match self.input.peek() {
                        None | Some('\n') | Some('\r') => break,
                        Some(_) => {
                            let _ = self.input.next();
                        }
                    }
                }
            } else if c == ' ' || c == '\t' {
                self.flush_text();
                // Whitespace is collected into a separate token so that the parser can trim.
                let mut whitespace = c.to_string();
                while let Some(' ' | '\t') = self.input.peek() {
                    whitespace.push(self.input.next().unwrap());
                }
                self.result.push(Token::Whitespace(whitespace));
            } else {
                self.text_buffer.push(c);
            }
        }

        self.flush_text();
        Ok(self.result)
    }

    /// \n\r and \r\n should be treated as a single newline.
    fn maybe_consume_second_newline(&mut self, current: char) {
        let peek = self.input.peek();
        if (current == '\n' && peek == Some(&'\r')) || (current == '\r' && peek == Some(&'\n')) {
            self.input.next();
        }
    }

    fn flush_text(&mut self) {
        if !self.text_buffer.is_empty() {
            self.result.push(Token::Text(self.text_buffer.clone()));
        }
        self.text_buffer.clear();
    }
}

pub(crate) fn is_valid_identifier(chr: char) -> bool {
    chr.is_ascii_alphanumeric() || chr == '.' || chr == '_'
}

#[derive(Debug, Display, Error, PartialEq, Eq)]
pub enum LexError {
    #[display("Variable doesn't have a closing }}")]
    MissingCloseVariable,
    #[display("invalid char in variable name: {f0}")]
    InvalidVariableChar(char),
}

#[cfg(test)]
mod tests {
    use super::Token::*;
    use super::*;

    #[test]
    fn test_empty() {
        assert_lex("", &[]);
    }

    #[test]
    fn test_single_text() {
        assert_lex("Hello world!", &[t("Hello"), w(" "), t("world!")]);
    }

    #[test]
    fn test_key_value() {
        assert_lex("key=value", &[t("key"), Equals, t("value")]);
        assert_lex("key = value", &[t("key"), w(" "), Equals, w(" "), t("value")]);
        assert_lex("key:value", &[t("key"), Colon, t("value")]);
        assert_lex("key : value", &[t("key"), w(" "), Colon, w(" "), t("value")]);
    }

    #[test]
    fn test_adjacent_symbols() {
        assert_lex(":=", &[Colon, Equals]);
        assert_lex(" : = ", &[w(" "), Colon, w(" "), Equals, w(" ")]);
        assert_lex("foo : = bar", &[t("foo"), w(" "), Colon, w(" "), Equals, w(" "), t("bar")]);
    }

    #[test]
    fn test_newline() {
        assert_lex("a\nb\nc", &[t("a"), NewLine, t("b"), NewLine, t("c")]);
        assert_lex("a\n\rb\n\nc\r", &[t("a"), NewLine, t("b"), NewLine, NewLine, t("c"), NewLine]);
    }

    #[test]
    fn test_dollar_not_variable() {
        assert_lex("$", &[t("$")]);
        assert_lex("$$", &[t("$")]);
        assert_lex("$${foo}", &[t("${foo}")]);
        assert_lex("$foo", &[t("$foo")]);
    }

    #[test]
    fn test_variable() {
        assert_lex("${foo}", &[v("foo")]);
        assert_lex("${foo} ${bar}", &[v("foo"), w(" "), v("bar")]);
        assert_lex_error("${foo bar}", LexError::InvalidVariableChar(' '));
        assert_lex_error("${foo-bar}", LexError::InvalidVariableChar('-'));
        assert_lex_error("${foo", LexError::MissingCloseVariable);
    }

    #[test]
    fn test_comments() {
        assert_lex("foo # bar\nbaz", &[t("foo"), w(" "), NewLine, t("baz")]);
    }

    #[test]
    fn test_escaped_comment() {
        assert_lex("foo \\# bar", &[t("foo"), w(" "), t("#"), w(" "), t("bar")]);
    }

    #[test]
    fn test_escaped_newline() {
        assert_lex("foo\\\nbar", &[t("foobar")]);
        assert_lex("foo\\\rbar", &[t("foobar")]);
        assert_lex("foo\\\r\nbar", &[t("foobar")]);
    }

    #[test]
    fn test_escape_non_escapable() {
        assert_lex("foo\\bar", &[t("foo\\bar")]);
        assert_lex("foo\\${bar}", &[t("foo\\"), v("bar")]);
    }

    #[track_caller]
    fn assert_lex(input: &str, expected: &[Token]) {
        match Lexer::new(input).lex() {
            Ok(result) => assert_eq!(expected, result.as_slice(), "while lexing: {input}"),
            Err(err) => panic!("{input} failed to lex: {err:?}"),
        }
    }

    #[track_caller]
    fn assert_lex_error(input: &str, expected: LexError) {
        match Lexer::new(input).lex() {
            Ok(_) => panic!("{input} lexed, but should've failed"),
            Err(err) => assert_eq!(expected, err, "different error while lexing {input}"),
        }
    }

    fn t(text: &str) -> Token {
        Text(text.into())
    }

    fn v(name: &str) -> Token {
        Variable(name.into())
    }

    fn w(whitespace: &str) -> Token {
        Whitespace(whitespace.into())
    }
}
