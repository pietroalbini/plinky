mod lexer;
mod parser;

use crate::template::lexer::Lexer;
use crate::template::parser::Parser;
use plinky_macros::{Display, Error};
use std::collections::HashMap;

pub struct TemplateContext {
    variables: HashMap<String, String>,
}

impl TemplateContext {
    pub fn new() -> Self {
        Self { variables: HashMap::new() }
    }

    pub fn set_variable(&mut self, key: &str, value: &str) {
        self.variables.insert(key.into(), value.into());
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Template {
    parts: Vec<Part>,
}

impl Template {
    pub fn parse(mut input: &str) -> Result<Self, TemplateParseError> {
        Parser::new(Lexer::new(&mut input)).parse()
    }

    pub fn resolve(&self, context: &TemplateContext) -> Option<String> {
        let mut result = String::new();
        for part in &self.parts {
            match part {
                Part::RawText(lit) => result.push_str(lit),
                Part::Expression(expr) => result.push_str(expr.resolve(context)?),
            }
        }
        Some(result)
    }

    pub fn will_resolve(&self, context: &TemplateContext) -> bool {
        for part in &self.parts {
            match part {
                Part::RawText(_) => {}
                Part::Expression(expr) => {
                    if expr.resolve(context).is_none() {
                        return false;
                    }
                }
            }
        }
        true
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Part {
    RawText(String),
    Expression(Expression),
}

#[derive(Debug, PartialEq, Eq)]
enum Expression {
    Variable(String),
}

impl Expression {
    fn resolve<'a>(&self, context: &'a TemplateContext) -> Option<&'a str> {
        match self {
            Expression::Variable(var) => context.variables.get(var).map(|s| s.as_str()),
        }
    }
}

#[derive(Debug, Display, Error)]
pub enum TemplateParseError {
    #[display("]unexpected char: {f0}")]
    UnexpectedChar(char),
    #[display("unexpected {actual}, expected {expected}")]
    UnexpectedToken { actual: String, expected: &'static str },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve() {
        assert_resolve(&TemplateContext::new(), "Hello world!", "Hello world!");
        assert_not_resolve(&TemplateContext::new(), "Hello ${name}");

        let mut ctx = TemplateContext::new();
        ctx.set_variable("name", "Pietro");

        assert_resolve(&ctx, "Hello ${name}!", "Hello Pietro!");
    }

    #[track_caller]
    fn assert_resolve(ctx: &TemplateContext, template: &str, expected: &str) {
        let template = Template::parse(template).unwrap();
        assert_eq!(expected, template.resolve(ctx).expect("template did not resolve"));
        assert!(template.will_resolve(ctx));
    }

    #[track_caller]
    fn assert_not_resolve(ctx: &TemplateContext, template: &str) {
        let template = Template::parse(template).unwrap();
        assert!(template.resolve(ctx).is_none(), "template did resolve");
        assert!(!template.will_resolve(ctx));
    }
}
