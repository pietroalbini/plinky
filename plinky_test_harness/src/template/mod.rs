mod functions;
mod lexer;
mod parser;
mod serde;

use crate::template::lexer::Lexer;
use crate::template::parser::Parser;
pub use functions::{FunctionCallError, IntoTemplateFunction, TemplateFunction};
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

    pub fn resolve(&self, context: &TemplateContext) -> Result<String, TemplateResolveError> {
        self.resolve_with(context, &ResolveHooks::new())
    }

    pub fn resolve_with(
        &self,
        context: &TemplateContext,
        hooks: &ResolveHooks<'_>,
    ) -> Result<String, TemplateResolveError> {
        let mut result = String::new();
        for part in &self.parts {
            match part {
                Part::RawText(lit) => result.push_str(lit),
                Part::Expression(expr) => {
                    let mut resolved = expr.resolve(context)?;
                    if let Some(hook) = &hooks.expression_resolved {
                        resolved = Cow::Owned(hook(resolved.into_owned()));
                    }
                    result.push_str(resolved.as_str()?.as_ref());
                }
            }
        }
        Ok(result)
    }

    pub fn will_resolve(&self, context: &TemplateContext) -> bool {
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
    FunctionCall {
        name: String,
        params: Vec<Expression>,
    },
    TernaryOperator {
        condition: Box<Expression>,
        if_true: Box<Expression>,
        if_false: Box<Expression>,
    },
}

impl Expression {
    fn resolve<'a>(
        &'a self,
        context: &'a TemplateContext,
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

                if condition.as_bool()? { Ok(if_true) } else { Ok(if_false) }
            }

            Expression::FunctionCall { name, params } => {
                let function = context
                    .get_function(name)
                    .ok_or_else(|| TemplateResolveError::MissingFunction(name.clone()))?;

                let mut resolved_params = Vec::new();
                for param in params {
                    resolved_params.push(param.resolve(context)?);
                }

                Ok(Cow::Owned(function.call(&resolved_params).map_err(|err| {
                    TemplateResolveError::FunctionCall { name: name.clone(), err }
                })?))
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
    fn as_str(&self) -> Result<Cow<'_, str>, ConversionError> {
        match self {
            Value::String(string) => Ok(Cow::Borrowed(string.as_str())),
            Value::Path(path) => match path.to_str() {
                Some(path) => Ok(Cow::Borrowed(path)),
                None => Err(ConversionError::NonUtf8Path(path.clone())),
            },
            Value::Bool(_) => Err(ConversionError::BoolToStringUnsupported),
        }
    }

    fn as_bool(&self) -> Result<bool, ConversionError> {
        match self {
            Value::String(_) => Err(ConversionError::StringToBoolUnsupported),
            Value::Path(_) => Err(ConversionError::PathToBoolUnsupported),
            Value::Bool(b) => Ok(*b),
        }
    }
}

pub struct TemplateContext {
    variables: HashMap<String, Value>,
    functions: HashMap<String, Box<dyn TemplateFunction>>,
}

impl TemplateContext {
    pub fn new() -> Self {
        Self { variables: HashMap::new(), functions: HashMap::new() }
    }

    pub fn set_variable(&mut self, key: &str, value: Value) {
        self.variables.insert(key.into(), value);
    }

    pub fn add_function<T, U>(&mut self, name: &str, func: T)
    where
        T: IntoTemplateFunction<U>,
    {
        self.functions.insert(name.into(), func.into_template_function());
    }

    fn get_variable(&self, key: &str) -> Option<Cow<'_, Value>> {
        self.variables.get(key).map(Cow::Borrowed)
    }

    fn get_function(&self, name: &str) -> Option<&dyn TemplateFunction> {
        self.functions.get(name).map(|v| &**v)
    }
}

pub struct ResolveHooks<'a> {
    expression_resolved: Option<Box<dyn Fn(Value) -> Value + 'a>>,
}

impl<'a> ResolveHooks<'a> {
    pub fn new() -> Self {
        Self { expression_resolved: None }
    }

    pub fn expression_resolved(mut self, f: impl Fn(Value) -> Value + 'a) -> Self {
        self.expression_resolved = Some(Box::new(f));
        self
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
    #[display("missing function: {f0}")]
    MissingFunction(String),
    #[display("cannot compare different types {f0:?} and {f1:?}")]
    TypeMismatchComparison(Value, Value),
    #[transparent]
    Conversion(ConversionError),
    #[display("calling function {name} failed")]
    FunctionCall {
        name: String,
        #[source]
        err: FunctionCallError,
    },
}

#[derive(Debug, Display, Error, PartialEq, Eq)]
pub enum ConversionError {
    #[display("non-UTF-8 path: {f0:?}")]
    NonUtf8Path(PathBuf),
    #[display("converting from bool to string is unsupported")]
    BoolToStringUnsupported,
    #[display("converting from bool to path is unsupported")]
    BoolToPathUnsupported,
    #[display("converting from string to bool is unsupported")]
    StringToBoolUnsupported,
    #[display("converting from path to bool is unsupported")]
    PathToBoolUnsupported,
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
            TemplateResolveError::Conversion(ConversionError::StringToBoolUnsupported),
        );
        assert_resolve_error(
            &ctx,
            "${ path ? 'yes' : 'no' }",
            TemplateResolveError::Conversion(ConversionError::PathToBoolUnsupported),
        );

        ctx.add_function("upper", |name: String| -> _ { Value::String(name.to_uppercase()) });
        assert_resolve(&ctx, "${upper(name)}", "PIETRO");
        assert_resolve_error(
            &ctx,
            "${upper()}",
            TemplateResolveError::FunctionCall {
                name: "upper".into(),
                err: FunctionCallError::TooFewArgs,
            },
        );
        assert_resolve_error(
            &ctx,
            "${upper(name, name)}",
            TemplateResolveError::FunctionCall {
                name: "upper".into(),
                err: FunctionCallError::TooManyArgs,
            },
        );
        assert_resolve_error(
            &ctx,
            "${upper(false)}",
            TemplateResolveError::FunctionCall {
                name: "upper".into(),
                err: FunctionCallError::InvalidArg {
                    position: 1,
                    err: ConversionError::BoolToStringUnsupported,
                },
            },
        );
        assert_resolve_error(
            &ctx,
            "${lower(name)}",
            TemplateResolveError::MissingFunction("lower".into()),
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
