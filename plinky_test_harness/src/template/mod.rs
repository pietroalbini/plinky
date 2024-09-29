mod lexer;
mod parser;
mod serde;

use crate::template::lexer::Lexer;
use crate::template::parser::Parser;
use plinky_macros::{Display, Error};
use std::borrow::Cow;
use std::collections::HashMap;
use std::mem::discriminant;
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
                Part::Expression(expr) => {
                    result.push_str(expr.resolve(context)?.as_str()?.as_ref());
                }
            }
        }
        Ok(result)
    }

    pub fn will_resolve(&self, context: &dyn TemplateContextGetters) -> bool {
        for part in &self.parts {
            match part {
                Part::RawText(_) => {}
                Part::Expression(expr) => match expr.resolve(context) {
                    Err(TemplateResolveError::MissingVariable(_)) => return false,
                    _ => {}
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
    Value(Value),
    Equals(Box<Expression>, Box<Expression>),
    TernaryOperator {
        condition: Box<Expression>,
        if_true: Box<Expression>,
        if_false: Box<Expression>,
    },
}

impl Expression {
    fn resolve<'a>(
        &'a self,
        context: &'a dyn TemplateContextGetters,
    ) -> Result<Cow<'a, Value>, TemplateResolveError> {
        match self {
            Expression::Variable(var) => context
                .get_variable(var)
                .ok_or_else(|| TemplateResolveError::MissingVariable(var.clone())),

            Expression::Value(value) => Ok(Cow::Borrowed(value)),

            Expression::Equals(lhs, rhs) => {
                let lhs = lhs.resolve(context)?;
                let rhs = rhs.resolve(context)?;

                self.type_check_comparison(&lhs, &rhs)?;
                Ok(Cow::Owned(Value::Bool(lhs.as_ref() == rhs.as_ref())))
            }

            Expression::TernaryOperator { condition, if_true, if_false } => {
                let condition = condition.resolve(context)?;
                let if_true = if_true.resolve(context)?;
                let if_false = if_false.resolve(context)?;

                if condition.as_bool()? {
                    Ok(if_true)
                } else {
                    Ok(if_false)
                }
            }
        }
    }

    fn type_check_comparison(&self, lhs: &Value, rhs: &Value) -> Result<(), TemplateResolveError> {
        if discriminant(lhs) != discriminant(rhs) {
            Err(TemplateResolveError::TypeMismatchComparison(lhs.clone(), rhs.clone()))
        } else {
            Ok(())
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    String(String),
    Path(PathBuf),
    Bool(bool),
}

impl Value {
    fn as_str(&self) -> Result<Cow<'_, str>, TemplateResolveError> {
        match self {
            Value::String(string) => Ok(Cow::Borrowed(string.as_str())),
            Value::Path(path) => match path.to_str() {
                Some(path) => Ok(Cow::Borrowed(path)),
                None => Err(TemplateResolveError::NonUtf8Path(path.clone())),
            },
            Value::Bool(_) => Err(TemplateResolveError::BoolToStringUnsupported),
        }
    }

    fn as_bool(&self) -> Result<bool, TemplateResolveError> {
        match self {
            Value::String(_) => Err(TemplateResolveError::StringToBoolUnsupported),
            Value::Path(_) => Err(TemplateResolveError::PathToBoolUnsupported),
            Value::Bool(b) => Ok(*b),
        }
    }
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

#[derive(Debug, Display, Error, PartialEq, Eq, Clone)]
pub enum TemplateParseError {
    #[display("unexpected char: {f0}")]
    UnexpectedChar(char),
    #[display("unexpected {actual}, expected {expected}")]
    UnexpectedToken { actual: String, expected: &'static str },
    #[display("unterminated string literal")]
    UnterminatedStringLiteral,
}

#[derive(Debug, Display, Error, PartialEq, Eq)]
pub enum TemplateResolveError {
    #[display("missing variable: {f0}")]
    MissingVariable(String),
    #[display("non-UTF-8 path: {f0:?}")]
    NonUtf8Path(PathBuf),
    #[display("converting from bool to string is unsupported")]
    BoolToStringUnsupported,
    #[display("converting from string to bool is unsupported")]
    StringToBoolUnsupported,
    #[display("converting from path to bool is unsupported")]
    PathToBoolUnsupported,
    #[display("cannot compare different types {f0:?} and {f1:?}")]
    TypeMismatchComparison(Value, Value),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve() {
        assert_resolve(&TemplateContext::new(), "Hello world!", "Hello world!");
        assert_not_resolvable(&TemplateContext::new(), "Hello ${name}");

        let mut ctx = TemplateContext::new();
        ctx.set_variable("name", Value::String("Pietro".into()));
        ctx.set_variable("path", Value::Path("/dev/null".into()));

        assert_resolve(&ctx, "Hello ${name}!", "Hello Pietro!");
        assert_resolve(&ctx, "The destination is ${path}.", "The destination is /dev/null.");

        assert_resolve(
            &ctx,
            "Access ${ name == 'Pietro' ? 'granted' : 'denied' }",
            "Access granted",
        );
        assert_resolve(
            &ctx,
            "Access ${ name == 'Someone else' ? 'granted' : 'denied' }",
            "Access denied",
        );

        assert_resolve_error(
            &ctx,
            "${ name == path ? 'yes' : 'no' }",
            TemplateResolveError::TypeMismatchComparison(
                Value::String("Pietro".into()),
                Value::Path("/dev/null".into()),
            ),
        );
        assert_resolve_error(
            &ctx,
            "${ name == true ? 'yes' : 'no' }",
            TemplateResolveError::TypeMismatchComparison(
                Value::String("Pietro".into()),
                Value::Bool(true),
            ),
        );
        assert_resolve_error(
            &ctx,
            "${ name ? 'yes' : 'no' }",
            TemplateResolveError::StringToBoolUnsupported,
        );
        assert_resolve_error(
            &ctx,
            "${ path ? 'yes' : 'no' }",
            TemplateResolveError::PathToBoolUnsupported,
        );
    }

    #[track_caller]
    fn assert_resolve(ctx: &TemplateContext, template: &str, expected: &str) {
        let template = Template::parse(template).unwrap();
        assert_eq!(expected, template.resolve(ctx).expect("template did not resolve"));
        assert!(template.will_resolve(ctx));
    }

    #[track_caller]
    fn assert_resolve_error(ctx: &TemplateContext, template: &str, err: TemplateResolveError) {
        let template = Template::parse(template).unwrap();
        assert!(template.will_resolve(ctx));
        assert_eq!(template.resolve(ctx).unwrap_err(), err);
    }

    #[track_caller]
    fn assert_not_resolvable(ctx: &TemplateContext, template: &str) {
        let template = Template::parse(template).unwrap();
        assert!(template.resolve(ctx).is_err(), "template did resolve");
        assert!(!template.will_resolve(ctx));
    }
}
