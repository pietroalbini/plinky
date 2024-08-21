use super::lexer::Token;
use super::Expression;
use crate::template::lexer::Lexer;
use crate::template::{Part, Template, TemplateParseError};

pub(super) struct Parser<'a, 'b> {
    lexer: Lexer<'a, 'b>,
}

impl<'a, 'b> Parser<'a, 'b> {
    pub(super) fn new(lexer: Lexer<'a, 'b>) -> Self {
        Self { lexer }
    }

    pub(super) fn parse(mut self) -> Result<Template, TemplateParseError> {
        let mut parts = Vec::new();

        while let Some(token) = self.lexer.next() {
            match token? {
                Token::RawText(raw) => parts.push(Part::RawText(raw.into())),
                Token::BeginInterpolation => {
                    // First check for the presence of a variable.
                    match self.lexer.next().transpose()? {
                        Some(Token::Variable(var)) => {
                            parts.push(Part::Expression(Expression::Variable(var.into())));
                        }
                        actual => return unexpected(actual, "variable"),
                    }

                    // Then for the presence of the interpolation end.
                    match self.lexer.next().transpose()? {
                        Some(Token::EndInterpolation) => {}
                        actual => return unexpected(actual, "end of interpolation"),
                    }
                }
                actual => return unexpected(actual, "raw text or beginning of interpolation"),
            }
        }

        Ok(Template { parts })
    }
}

fn unexpected<'a, T>(
    token: impl Into<Option<Token<'a>>>,
    expected: &'static str,
) -> Result<T, TemplateParseError> {
    Err(TemplateParseError::UnexpectedToken {
        actual: match token.into() {
            Some(token) => token.to_string(),
            None => "end of template".into(),
        },
        expected,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_just_raw() {
        assert_eq!(
            Template { parts: vec![Part::RawText("Hello world!".into())] },
            parse("Hello world!").unwrap()
        );
    }

    #[test]
    fn test_parse_one_variable() {
        assert_eq!(
            Template { parts: vec![raw("Hello "), var("name"), raw("!")] },
            parse("Hello ${name}!").unwrap()
        );
    }

    #[test]
    fn test_parse_two_variables() {
        assert_eq!(
            Template {
                parts: vec![
                    raw("Hello "),
                    var("name"),
                    raw(" I am "),
                    var("caller.name"),
                    raw("!"),
                ]
            },
            parse("Hello ${name} I am ${ caller.name }!").unwrap()
        );
    }

    #[test]
    fn test_bad_templates() {
        let bad = &["Hello ${", "Hello ${ var", "Hello ${var1 var2"];

        for template in bad {
            assert!(parse(template).is_err(), "template is valid but shouldn't: {template}");
        }
    }

    fn raw(text: &str) -> Part {
        Part::RawText(text.into())
    }

    fn var(name: &str) -> Part {
        Part::Expression(Expression::Variable(name.into()))
    }

    fn parse(mut input: &str) -> Result<Template, TemplateParseError> {
        Parser::new(Lexer::new(&mut input)).parse()
    }
}
