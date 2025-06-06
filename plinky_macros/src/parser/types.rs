use crate::error::Error;
use plinky_utils::quote::Quote;
use proc_macro::{Span, TokenStream, TokenTree};

#[derive(Debug, Clone)]
pub(crate) enum Item {
    Struct(Struct),
    Enum(Enum),
}

#[derive(Debug, Clone)]
pub(crate) struct Struct {
    pub(crate) attrs: Attributes,
    pub(crate) name: Ident,
    pub(crate) generics: Vec<GenericParam>,
    pub(crate) fields: StructFields,
    pub(crate) span: Span,
}

#[derive(Debug, Clone)]
pub(crate) enum StructFields {
    None,
    TupleLike(Vec<TupleField>),
    StructLike(Vec<StructField>),
}

#[derive(Debug, Clone)]
pub(crate) struct StructField {
    pub(crate) attrs: Attributes,
    pub(crate) name: Ident,
    pub(crate) ty: Type,
}

#[derive(Debug, Clone)]
pub(crate) struct Enum {
    pub(crate) _attrs: Attributes,
    pub(crate) name: Ident,
    pub(crate) generics: Vec<GenericParam>,
    pub(crate) variants: Vec<EnumVariant>,
}

#[derive(Debug, Clone)]
pub(crate) struct EnumVariant {
    pub(crate) span: Span,
    pub(crate) attrs: Attributes,
    pub(crate) name: Ident,
    pub(crate) data: EnumVariantData,
}

#[derive(Debug, Clone)]
pub(crate) enum EnumVariantData {
    None,
    TupleLike(Vec<TupleField>),
    StructLike(Vec<StructField>),
}

#[derive(Debug, Clone)]
pub(crate) struct TupleField {
    pub(crate) attrs: Attributes,
    pub(crate) ty: Type,
}

#[derive(Debug, Clone)]
pub(crate) enum GenericParam {
    Normal(GenericParamNormal),
    Const(GenericParamConst),
}

#[derive(Debug, Clone)]
pub(crate) struct GenericParamNormal {
    pub(crate) name: Ident,
    pub(crate) bound: TokenStream,
}

#[derive(Debug, Clone)]
pub(crate) struct GenericParamConst {
    pub(crate) name: Ident,
    pub(crate) type_: Type,
    pub(crate) _default: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct Type(pub(crate) TokenStream);

impl Quote for Type {
    fn to_token_stream(&self) -> TokenStream {
        self.0.clone()
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Ident {
    pub(crate) name: String,
    pub(crate) span: Span,
}

impl Quote for Ident {
    fn to_token_stream(&self) -> TokenStream {
        TokenStream::from(TokenTree::Ident(proc_macro::Ident::new(&self.name, self.span)))
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Attributes {
    pub(super) attributes: Vec<Attribute>,
}

impl Attributes {
    pub(crate) fn get(&self, name: &str) -> Result<Option<&Attribute>, Error> {
        let mut result = None;
        for attr in &self.attributes {
            if attr.name == name {
                if result.is_some() {
                    return Err(Error::new("duplicate attribute").span(attr.span));
                } else {
                    result = Some(attr);
                }
            }
        }
        Ok(result)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Attribute {
    pub(crate) span: Span,
    pub(crate) name: String,
    pub(crate) content: AttributeContent,
}

impl Attribute {
    pub(crate) fn must_be_empty(&self) -> Result<(), Error> {
        if let AttributeContent::Empty = &self.content {
            Ok(())
        } else {
            Err(Error::new("attribute must not have any value").span(self.span))
        }
    }

    pub(crate) fn get_equals_to_str(&self) -> Result<&str, Error> {
        if let AttributeContent::EqualsTo(AttributeValue::String(string)) = &self.content {
            return Ok(string);
        }
        Err(Error::new("expected attribute to have one string after =").span(self.span))
    }

    pub(crate) fn get_parenthesis_one_str(&self) -> Result<&str, Error> {
        if let AttributeContent::ParenthesisList(list) = &self.content {
            if let [AttributeValue::String(string)] = list.as_slice() {
                return Ok(string);
            }
        }
        Err(Error::new("expected attribute to have one quoted string inside parenthesis")
            .span(self.span))
    }

    pub(crate) fn get_parenthesis_one_expr(&self) -> Result<&TokenStream, Error> {
        if let AttributeContent::ParenthesisList(list) = &self.content {
            if let [AttributeValue::Expr(expr)] = list.as_slice() {
                return Ok(expr);
            }
        }
        Err(Error::new("expected attribute to have one expression inside parenthesis")
            .span(self.span))
    }
}

#[derive(Debug, Clone)]
pub(crate) enum AttributeContent {
    Empty,
    EqualsTo(AttributeValue),
    ParenthesisList(Vec<AttributeValue>),
}

#[derive(Debug, Clone)]
pub(crate) enum AttributeValue {
    Expr(TokenStream),
    String(String),
}
