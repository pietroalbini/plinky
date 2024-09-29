use super::lexer::Token;
use super::{Expression, Value};
use crate::template::lexer::Lexer;
use crate::template::{Part, Template, TemplateParseError};
use std::iter::Peekable;

pub(super) struct Parser<'a, 'b> {
    lexer: Peekable<Lexer<'a, 'b>>,
}

impl<'a, 'b> Parser<'a, 'b> {
    pub(super) fn new(lexer: Lexer<'a, 'b>) -> Self {
        Self { lexer: lexer.peekable() }
    }

    pub(super) fn parse(mut self) -> Result<Template, TemplateParseError> {
        let mut parts = Vec::new();

        while let Some(token) = self.lexer.next() {
            match token? {
                Token::RawText(raw) => {
                    if !raw.is_empty() {
                        parts.push(Part::RawText(raw.into()))
                    }
                }
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
        let lhs = self.parse_expression_1()?;

        if self.peek()? == Some(&Token::QuestionMark) {
            self.lexer.next().transpose()?;

            let if_true = self.parse_expression_1()?;
            self.expect(Token::Colon, ":")?;
            let if_false = self.parse_expression_1()?;

            Ok(Expression::TernaryOperator {
                condition: Box::new(lhs),
                if_true: Box::new(if_true),
                if_false: Box::new(if_false),
            })
        } else {
            Ok(lhs)
        }
    }

    pub(super) fn parse_expression_1(&mut self) -> Result<Expression, TemplateParseError> {
        let lhs = self.parse_expression_2()?;

        if self.peek()? == Some(&Token::DoubleEquals) {
            self.lexer.next().transpose()?;

            let rhs = self.parse_expression_2()?;
            Ok(Expression::Equals(Box::new(lhs), Box::new(rhs)))
        } else {
            Ok(lhs)
        }
    }

    pub(super) fn parse_expression_2(&mut self) -> Result<Expression, TemplateParseError> {
        match self.lexer.next().transpose()? {
            Some(Token::Variable("true")) => Ok(Expression::Value(Value::Bool(true))),
            Some(Token::Variable("false")) => Ok(Expression::Value(Value::Bool(false))),
            Some(Token::Variable(var)) => Ok(Expression::Variable(var.into())),

            Some(Token::StringLiteral(lit)) => Ok(Expression::Value(Value::String(lit.into()))),

            actual => unexpected(actual, "variable or value"),
        }
    }

    fn expect(
        &mut self,
        token: Token<'_>,
        expected: &'static str,
    ) -> Result<(), TemplateParseError> {
        match self.lexer.next().transpose()? {
            Some(actual) if actual == token => Ok(()),
            other => unexpected(other, expected),
        }
    }

    fn peek(&mut self) -> Result<Option<&Token>, TemplateParseError> {
        match self.lexer.peek() {
            Some(Ok(token)) => Ok(Some(token)),
            Some(Err(err)) => Err(err.clone()),
            None => Ok(None),
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
    fn test_parse_empty() {
        assert_eq!(template([]), parse("").unwrap());
    }

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
            template([raw("Hello "), var("name"), raw(" I am "), var("caller.name"), raw("!")]),
            parse("Hello ${name} I am ${ caller.name }!").unwrap()
        );
    }

    #[test]
    fn test_parse_booleans() {
        assert_eq!(
            template([bool_lit(true), bool_lit(false), var("maybe")]),
            parse("${ true }${ false }${ maybe }").unwrap(),
        );
    }

    #[test]
    fn test_parse_equals() {
        assert_eq!(
            template([eq(var("kind"), str_lit("hello"))]),
            parse("${ kind == 'hello' }").unwrap()
        );
    }

    #[test]
    fn test_parse_ternary() {
        assert_eq!(
            template([
                raw("Bits: "),
                ternary(eq(var("arch"), str_lit("x86_64")), str_lit("32"), str_lit("64")),
            ]),
            parse("Bits: ${ arch == 'x86_64' ? '32' : '64' }").unwrap(),
        )
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

    fn bool_lit(lit: bool) -> Part {
        Part::Expression(Expression::Value(Value::Bool(lit)))
    }

    fn eq(lhs: Part, rhs: Part) -> Part {
        Part::Expression(Expression::Equals(box_expr(lhs), box_expr(rhs)))
    }

    fn ternary(condition: Part, if_true: Part, if_false: Part) -> Part {
        Part::Expression(Expression::TernaryOperator {
            condition: box_expr(condition),
            if_true: box_expr(if_true),
            if_false: box_expr(if_false),
        })
    }

    fn box_expr(part: Part) -> Box<Expression> {
        match part {
            Part::Expression(expr) => Box::new(expr),
            _ => panic!("{part:?} is not an expression"),
        }
    }

    fn parse(mut input: &str) -> Result<Template, TemplateParseError> {
        Parser::new(Lexer::new(&mut input)).parse()
    }
}
