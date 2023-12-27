mod attributes;
mod types;
mod utils;

use crate::error::Error;
pub(crate) use crate::parser::types::*;
use crate::parser::utils::*;
use proc_macro::{Delimiter, Span, TokenStream, TokenTree};
use std::iter::Peekable;

pub(crate) struct Parser {
    tokens: Vec<Peekable<proc_macro::token_stream::IntoIter>>,
    last_span: Option<Span>,
}

impl Parser {
    pub(crate) fn new(stream: TokenStream) -> Self {
        Self { tokens: vec![stream.into_iter().peekable()], last_span: None }
    }

    pub(crate) fn parse_item(&mut self) -> Result<Item, Error> {
        let attrs = self.parse_attributes()?;
        self.skip_visibility()?;
        match self.next()? {
            next if next.is_ident("struct") => {
                Ok(Item::Struct(self.parse_struct_after_keyword(attrs)?))
            }
            next if next.is_ident("enum") => Ok(Item::Enum(self.parse_enum_after_keyword(attrs)?)),
            other => Err(Error::new("unexpected keyword").span(other.span())),
        }
    }

    pub(crate) fn parse_struct(&mut self) -> Result<Struct, Error> {
        let attrs = self.parse_attributes()?;
        self.skip_visibility()?;
        self.expect_keyword("struct")?;
        self.parse_struct_after_keyword(attrs)
    }

    fn parse_struct_after_keyword(&mut self, attrs: Attributes) -> Result<Struct, Error> {
        let (name, span) = match self.next()? {
            TokenTree::Ident(name) => (name.to_string(), name.span()),
            other => return Err(Error::new("expected struct name").span(other.span())),
        };

        let generics = self.parse_generic_params()?;

        let fields = match self.peek()? {
            TokenTree::Group(group) => {
                self.next()?;
                match group.delimiter() {
                    Delimiter::Parenthesis => {
                        StructFields::TupleLike(self.within_stream(group.stream(), |this| {
                            this.parse_comma_list(|this| Ok(this.parse_tuple_field()?.into()))
                        })?)
                    }
                    Delimiter::Brace => {
                        StructFields::StructLike(self.within_stream(group.stream(), |this| {
                            this.parse_comma_list(|this| Ok(this.parse_struct_field()?.into()))
                        })?)
                    }
                    _ => return Err(Error::new("expected struct content").span(group.span())),
                }
            }
            TokenTree::Punct(punct) if punct.as_char() == ';' => {
                self.next()?;
                StructFields::None
            }
            other => return Err(Error::new("expected struct content").span(other.span())),
        };

        Ok(Struct { attrs, name, generics, fields, span })
    }

    fn parse_struct_field(&mut self) -> Result<StructField, Error> {
        let attrs = self.parse_attributes()?;
        self.skip_visibility()?;
        let name = self.parse_ident()?;
        self.expect_punct(':')?;
        let ty = self.parse_type()?;
        Ok(StructField { attrs, name, ty })
    }

    fn parse_enum_after_keyword(&mut self, attrs: Attributes) -> Result<Enum, Error> {
        Ok(Enum {
            _attrs: attrs,
            name: self.parse_ident()?,
            generics: self.parse_generic_params()?,
            variants: self.within_braces(|this| {
                this.parse_comma_list(|this| Ok(this.parse_enum_variant()?.into()))
            })?,
        })
    }

    fn parse_enum_variant(&mut self) -> Result<EnumVariant, Error> {
        let attrs = self.parse_attributes()?;

        let (name, span) = match self.next()? {
            TokenTree::Ident(ident) => (ident.to_string(), ident.span()),
            other => return Err(Error::new("expected variant name").span(other.span())),
        };

        let data = if let Ok(TokenTree::Group(group)) = self.peek() {
            self.next()?;
            match group.delimiter() {
                Delimiter::Parenthesis => {
                    EnumVariantData::TupleLike(self.within_stream(group.stream(), |this| {
                        this.parse_comma_list(|this| Ok(this.parse_tuple_field()?.into()))
                    })?)
                }
                Delimiter::Brace => {
                    EnumVariantData::StructLike(self.within_stream(group.stream(), |this| {
                        this.parse_comma_list(|this| Ok(this.parse_struct_field()?.into()))
                    })?)
                }
                _ => return Err(Error::new("invalid enum variant").span(group.span())),
            }
        } else {
            EnumVariantData::None
        };

        Ok(EnumVariant { span, attrs, name, data })
    }

    fn parse_tuple_field(&mut self) -> Result<TupleField, Error> {
        let attrs = self.parse_attributes()?;
        let ty = self.parse_type()?;
        Ok(TupleField { attrs, ty })
    }

    fn parse_ident(&mut self) -> Result<String, Error> {
        match self.next()? {
            TokenTree::Ident(ident) => Ok(ident.to_string()),
            other => Err(Error::new("expected an ident").span(other.span())),
        }
    }

    fn parse_type(&mut self) -> Result<String, Error> {
        let mut ty = String::new();
        let mut repeat_without_new_segment = false;
        loop {
            match self.peek()? {
                // Generic, arrays or tuples.
                TokenTree::Group(group) => {
                    self.next()?;
                    ty.push_str(&group.to_string());
                }
                // Type names.
                TokenTree::Ident(ident) => {
                    self.next()?;
                    ty.push_str(&ident.to_string());
                    if self.peek().map(|t| t.is_punct('<')).unwrap_or(false) {
                        ty.push_str(&self.parse_generic_in_type_name()?);
                    }
                }
                TokenTree::Punct(punct) if punct.as_char() == '<' => {
                    ty.push_str(&self.parse_generic_in_type_name()?);
                }
                TokenTree::Punct(punct) if punct.as_char() == '&' => {
                    self.next()?;
                    ty.push('&');

                    // Lifetimes
                    if self.peek()?.is_punct('\'') {
                        self.next()?;
                        ty.push('\'');
                        match self.next()? {
                            TokenTree::Ident(ident) => ty.push_str(&ident.to_string()),
                            other => {
                                return Err(Error::new("expected lifetime name").span(other.span()));
                            }
                        }
                        ty.push(' ');
                    }

                    for keyword in ["mut", "dyn"] {
                        if self.peek()?.is_ident(keyword) {
                            self.next()?;
                            ty.push_str(keyword);
                            ty.push(' ');
                        }
                    }

                    repeat_without_new_segment = true;
                }
                other => return Err(Error::new("expected a type").span(other.span())),
            }

            if repeat_without_new_segment {
                repeat_without_new_segment = false;
            } else if self.peek().map(|p| p.is_punct(':')).unwrap_or(false) {
                self.next()?;
                self.expect_punct(':')?;
                ty.push_str("::");
            } else {
                break;
            }
        }
        Ok(ty)
    }

    fn parse_generic_in_type_name(&mut self) -> Result<String, Error> {
        self.expect_punct('<')?;
        let mut generic = "<".to_string();
        let mut count = 1;
        while count > 0 {
            let next = self.next()?;
            generic.push_str(&next.to_string());

            if next.is_punct('<') {
                count += 1;
            } else if next.is_punct('>') {
                count -= 1;
            }
        }

        Ok(generic)
    }

    fn parse_generic_params(&mut self) -> Result<Vec<GenericParam>, Error> {
        if !self.peek()?.is_punct('<') {
            return Ok(Vec::new());
        }
        self.next()?;

        let params = self.parse_comma_list(|this| {
            if this.peek()?.is_punct('>') {
                return Ok(IterationOutcome::Break);
            }

            let mut constructor: fn(GenericParam) -> _ = IterationOutcome::Value;
            if this.peek()?.is_ident("const") {
                this.next()?;

                let name = match this.next()? {
                    TokenTree::Ident(ident) => ident.to_string(),
                    other => return Err(Error::new("expected generic name").span(other.span())),
                };
                this.expect_punct(':')?;
                let type_ = this.parse_type()?;

                let default = if this.peek()?.is_punct('=') {
                    this.next()?;

                    let mut default = Vec::new();
                    loop {
                        if this.peek()?.is_punct('>') {
                            constructor = IterationOutcome::Last;
                            break;
                        } else if this.peek()?.is_punct(',') {
                            break;
                        } else {
                            default.push(this.next()?);
                        }
                    }

                    Some(default.into_iter().collect::<TokenStream>().to_string())
                } else {
                    if this.peek()?.is_punct('>') {
                        constructor = IterationOutcome::Last;
                    }
                    None
                };
                Ok(constructor(GenericParam::Const(GenericParamConst {
                    name,
                    type_,
                    _default: default,
                })))
            } else {
                let name = match this.next()? {
                    TokenTree::Ident(ident) => ident.to_string(),
                    other => return Err(Error::new("expected generic name").span(other.span())),
                };
                this.expect_punct(':')?;

                let mut bound = Vec::new();
                let mut constructor: fn(GenericParam) -> _ = IterationOutcome::Value;
                loop {
                    if this.peek()?.is_punct('>') {
                        constructor = IterationOutcome::Last;
                        break;
                    } else if this.peek()?.is_punct(',') {
                        break;
                    }
                    bound.push(this.next()?);
                }
                Ok(constructor(GenericParam::Normal(GenericParamNormal {
                    name,
                    bound: bound.into_iter().collect::<TokenStream>().to_string(),
                })))
            }
        })?;

        self.expect_punct('>')?;
        Ok(params)
    }

    fn skip_visibility(&mut self) -> Result<(), Error> {
        if self.peek()?.is_ident("pub") {
            self.next()?;
            if let TokenTree::Group(_) = self.peek()? {
                self.next()?;
            }
        }
        Ok(())
    }
}
