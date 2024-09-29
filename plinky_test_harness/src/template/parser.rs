use super::lexer::Token;
use super::{Expression, Value};
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
                    // First check for the expression.
                    parts.push(Part::Expression(self.parse_expression()?));

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

    pub(super) fn parse_expression(&mut self) -> Result<Expression, TemplateParseError> {
        match self.lexer.next().transpose()? {
            Some(Token::Variable(var)) => Ok(Expression::Variable(var.into())),
            Some(Token::StringLiteral(lit)) => Ok(Expression::Value(Value::String(lit.into()))),
            actual => unexpected(actual, "variable"),
        }
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
    use crate::template::Value;

    #[test]
    fn test_parse_just_raw() {
        assert_eq!(template([raw("Hello world!")]), parse("Hello world!").unwrap());
    }

    #[test]
    fn test_parse_one_variable() {
        assert_eq!(
            template([raw("Hello "), var("name"), raw("!")]),
            parse("Hello ${name}!").unwrap()
        );
    }

    #[test]
    fn test_parse_one_string_literal() {
        assert_eq!(
            template([raw("Hello "), str_lit("Pietro"), raw("!")]),
            parse("Hello ${'Pietro'}!").unwrap()
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

    fn template<const N: usize>(parts: [Part; N]) -> Template {
        Template { parts: parts.into() }
    }

    fn raw(text: &str) -> Part {
        Part::RawText(text.into())
    }

    fn var(name: &str) -> Part {
        Part::Expression(Expression::Variable(name.into()))
    }

    fn str_lit(lit: &str) -> Part {
        Part::Expression(Expression::Value(Value::String(lit.into())))
    }

    fn parse(mut input: &str) -> Result<Template, TemplateParseError> {
        Parser::new(Lexer::new(&mut input)).parse()
    }
}
