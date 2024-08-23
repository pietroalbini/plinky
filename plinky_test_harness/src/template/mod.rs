mod lexer;
mod parser;
mod serde;

use crate::template::lexer::Lexer;
use crate::template::parser::Parser;
use plinky_macros::{Display, Error};
use std::borrow::Cow;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Template {
    parts: Vec<Part>,
}

impl Template {
    pub fn parse(mut input: &str) -> Result<Self, TemplateParseError> {
        Parser::new(Lexer::new(&mut input)).parse()
    }

    pub fn resolve(
        &self,
        context: &dyn TemplateContextGetters,
    ) -> Result<String, TemplateResolveError> {
        let mut result = String::new();
        for part in &self.parts {
            match part {
                Part::RawText(lit) => result.push_str(lit),
                Part::Expression(expr) => match expr.resolve(context)?.as_ref() {
                    Value::String(s) => result.push_str(s),
                    Value::Path(p) => result.push_str(
                        p.to_str().ok_or_else(|| TemplateResolveError::NonUtf8Path(p.clone()))?,
                    ),
                },
            }
        }
        Ok(result)
    }

    pub fn will_resolve(&self, context: &dyn TemplateContextGetters) -> bool {
        for part in &self.parts {
            match part {
                Part::RawText(_) => {}
                Part::Expression(expr) => match expr.resolve(context) {
                    Ok(_) => {}
                    Err(TemplateResolveError::NonUtf8Path(_)) => {}
                    Err(TemplateResolveError::MissingVariable(_)) => return false,
                },
            }
        }
        true
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
enum Part {
    RawText(String),
    Expression(Expression),
}

#[derive(Debug, PartialEq, Eq, Clone)]
enum Expression {
    Variable(String),
}

impl Expression {
    fn resolve<'a>(
        &self,
        context: &'a dyn TemplateContextGetters,
    ) -> Result<Cow<'a, Value>, TemplateResolveError> {
        match self {
            Expression::Variable(var) => context
                .get_variable(var)
                .ok_or_else(|| TemplateResolveError::MissingVariable(var.clone())),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Value {
    String(String),
    Path(PathBuf),
}

pub struct TemplateContext {
    variables: HashMap<String, Value>,
}

impl TemplateContext {
    pub fn new() -> Self {
        Self { variables: HashMap::new() }
    }

    pub fn set_variable(&mut self, key: &str, value: Value) {
        self.variables.insert(key.into(), value);
    }
}

pub trait TemplateContextGetters {
    fn get_variable(&self, key: &str) -> Option<Cow<'_, Value>>;
}

impl TemplateContextGetters for TemplateContext {
    fn get_variable(&self, key: &str) -> Option<Cow<'_, Value>> {
        self.variables.get(key).map(Cow::Borrowed)
    }
}

#[derive(Debug, Display, Error)]
pub enum TemplateParseError {
    #[display("unexpected char: {f0}")]
    UnexpectedChar(char),
    #[display("unexpected {actual}, expected {expected}")]
    UnexpectedToken { actual: String, expected: &'static str },
}

#[derive(Debug, Display, Error)]
pub enum TemplateResolveError {
    #[display("missing variable: {f0}")]
    MissingVariable(String),
    #[display("non-UTF-8 path: {f0:?}")]
    NonUtf8Path(PathBuf),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve() {
        assert_resolve(&TemplateContext::new(), "Hello world!", "Hello world!");
        assert_not_resolve(&TemplateContext::new(), "Hello ${name}");

        let mut ctx = TemplateContext::new();
        ctx.set_variable("name", Value::String("Pietro".into()));
        ctx.set_variable("path", Value::Path("/dev/null".into()));

        assert_resolve(&ctx, "Hello ${name}!", "Hello Pietro!");
        assert_resolve(&ctx, "The destination is ${path}.", "The destination is /dev/null.");
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
        assert!(template.resolve(ctx).is_err(), "template did resolve");
        assert!(!template.will_resolve(ctx));
    }
}
