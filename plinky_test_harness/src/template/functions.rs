use crate::template::{ConversionError, Value};
use plinky_macros::{Display, Error};
use std::borrow::Cow;
use std::path::PathBuf;

pub trait TemplateFunction {
    fn call(&self, params: &[Cow<'_, Value>]) -> Result<Value, FunctionCallError>;
}

impl<F> TemplateFunction for F
where
    F: Fn(&[Cow<'_, Value>]) -> Result<Value, FunctionCallError>,
{
    fn call(&self, params: &[Cow<'_, Value>]) -> Result<Value, FunctionCallError> {
        (*self)(params)
    }
}

// Unfortunately it's not possible to define a trait implementation like this:
//
// ```rust
// impl<T, P1> TemplateFunction for T
// where
//     T: Fn(P1) -> Value,
//     P1: IntoValue,
// ```
//
// That's because the type system wants P1 to be present either in Self, the trait name, or the
// type name. In this case it is an error because in theory there could be multiple different Fn
// implementations for the trait.
//
// To work around that, we create an intermediate IntoTemplateFunction that is generic over all the
// params of the function, which returns a boxed TemplateFunction (by returning a closure). This
// allows for P1 to be present in the trait name and then erase it a TemplateFunction.
pub trait IntoTemplateFunction<T> {
    fn into_template_function(self) -> Box<dyn TemplateFunction>;
}

macro_rules! impl_template_function {
    ($($param:ident),*) => {
        impl<F, $($param),*> IntoTemplateFunction<($($param,)*)> for F
        where
            F: Fn($($param),*) -> Value + 'static,
            $($param: FromValue),*
        {
            #[allow(non_snake_case)]
            fn into_template_function(self) -> Box<dyn TemplateFunction> {
                Box::new(move |params: &[Cow<'_, Value>]| -> Result<Value, FunctionCallError> {
                    let mut params = params.iter();
                    let mut position = 0;

                    $(
                        position += 1;
                        let $param = $param::from_value(
                            params.next().
                            ok_or(FunctionCallError::TooFewArgs)?
                        ).map_err(|err| FunctionCallError::InvalidArg { position, err })?;
                    )*

                    if params.next().is_some() {
                        return Err(FunctionCallError::TooManyArgs);
                    }

                    Ok(self($($param),*))
                })
            }
        }
    }
}

// Implement TemplateFunction for multiple arguments.
impl_template_function!(P1);
impl_template_function!(P1, P2);
impl_template_function!(P1, P2, P3);
impl_template_function!(P1, P2, P3, P4);
impl_template_function!(P1, P2, P3, P4, P5);
impl_template_function!(P1, P2, P3, P4, P5, P6);
impl_template_function!(P1, P2, P3, P4, P5, P6, P7);
impl_template_function!(P1, P2, P3, P4, P5, P6, P7, P8);

trait FromValue: Sized {
    fn from_value(value: &Cow<'_, Value>) -> Result<Self, ConversionError>;
}

impl FromValue for String {
    fn from_value(value: &Cow<'_, Value>) -> Result<Self, ConversionError> {
        value.as_str().map(|s| s.to_string())
    }
}

impl FromValue for bool {
    fn from_value(value: &Cow<'_, Value>) -> Result<Self, ConversionError> {
        value.as_bool()
    }
}

impl FromValue for PathBuf {
    fn from_value(value: &Cow<'_, Value>) -> Result<Self, ConversionError> {
        match value.as_ref() {
            Value::String(s) => Ok(s.into()),
            Value::Path(p) => Ok(p.clone()),
            Value::Bool(_) => Err(ConversionError::BoolToPathUnsupported),
        }
    }
}

#[derive(Debug, Error, Display, PartialEq, Eq)]
pub enum FunctionCallError {
    #[display("too few arguments")]
    TooFewArgs,
    #[display("too many arguments")]
    TooManyArgs,
    #[display("invalid function argument {position}")]
    InvalidArg {
        position: usize,
        #[source]
        err: ConversionError,
    },
}
