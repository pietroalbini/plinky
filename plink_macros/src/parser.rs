use crate::error::Error;
use proc_macro::{Delimiter, Span, TokenStream, TokenTree};
use std::iter::Peekable;

#[derive(Debug)]
pub(crate) enum Item {
    Struct(Struct),
    Enum(Enum),
}

impl Item {
    pub(crate) fn name(&self) -> &str {
        match self {
            Item::Struct(struct_) => &struct_.name,
            Item::Enum(enum_) => &enum_.name,
        }
    }
}

#[derive(Debug)]
pub(crate) struct Struct {
    pub(crate) name: String,
    pub(crate) fields: StructFields,
    pub(crate) span: Span,
}

#[derive(Debug)]
pub(crate) enum StructFields {
    None,
    TupleLike(Vec<TupleField>),
    StructLike(Vec<StructField>),
}

#[derive(Debug)]
pub(crate) struct StructField {
    pub(crate) attrs: Vec<Attribute>,
    pub(crate) name: String,
    pub(crate) ty: String,
}

#[derive(Debug)]
pub(crate) struct Attribute {
    pub(crate) span: Span,
    pub(crate) value: String,
}

#[derive(Debug)]
pub(crate) struct Enum {
    pub(crate) name: String,
    pub(crate) variants: Vec<EnumVariant>,
}

#[derive(Debug)]
pub(crate) struct EnumVariant {
    pub(crate) span: Span,
    pub(crate) name: String,
    pub(crate) data: EnumVariantData,
}

#[derive(Debug)]
pub(crate) enum EnumVariantData {
    None,
    TupleLike(Vec<TupleField>),
    StructLike(Vec<StructField>),
}

#[derive(Debug)]
pub(crate) struct TupleField {
    pub(crate) attrs: Vec<Attribute>,
    pub(crate) ty: String,
}

pub(crate) struct Parser {
    tokens: Vec<Peekable<proc_macro::token_stream::IntoIter>>,
    last_span: Option<Span>,
}

impl Parser {
    pub(crate) fn new(stream: TokenStream) -> Self {
        Self {
            tokens: vec![stream.into_iter().peekable()],
            last_span: None,
        }
    }

    pub(crate) fn parse_item(&mut self) -> Result<Item, Error> {
        self.skip_visibility()?;
        match self.next()? {
            next if next.is_ident("struct") => Ok(Item::Struct(self.parse_struct_after_keyword()?)),
            next if next.is_ident("enum") => Ok(Item::Enum(self.parse_enum_after_keyword()?)),
            other => Err(Error::new("unexpected keyword").span(other.span())),
        }
    }

    pub(crate) fn parse_struct(&mut self) -> Result<Struct, Error> {
        self.skip_visibility()?;
        self.expect_keyword("struct")?;
        self.parse_struct_after_keyword()
    }

    fn parse_struct_after_keyword(&mut self) -> Result<Struct, Error> {
        let (name, span) = match self.next()? {
            TokenTree::Ident(name) => (name.to_string(), name.span()),
            other => return Err(Error::new("expected struct name").span(other.span())),
        };

        let fields = match self.peek()? {
            TokenTree::Group(group) => {
                self.next()?;
                match group.delimiter() {
                    Delimiter::Parenthesis => {
                        StructFields::TupleLike(self.within_stream(group.stream(), |this| {
                            this.parse_comma_list(|this| this.parse_tuple_field())
                        })?)
                    }
                    Delimiter::Brace => {
                        StructFields::StructLike(self.within_stream(group.stream(), |this| {
                            this.parse_comma_list(|this| this.parse_struct_field())
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

        Ok(Struct { name, fields, span })
    }

    fn parse_struct_field(&mut self) -> Result<StructField, Error> {
        let attrs = self.parse_attributes()?;
        self.skip_visibility()?;
        let name = self.parse_ident()?;
        self.expect_punct(':')?;
        let ty = self.parse_type()?;
        Ok(StructField { attrs, name, ty })
    }

    fn parse_enum_after_keyword(&mut self) -> Result<Enum, Error> {
        Ok(Enum {
            name: self.parse_ident()?,
            variants: self
                .within_braces(|this| this.parse_comma_list(|this| this.parse_enum_variant()))?,
        })
    }

    fn parse_enum_variant(&mut self) -> Result<EnumVariant, Error> {
        let (name, span) = match self.next()? {
            TokenTree::Ident(ident) => (ident.to_string(), ident.span()),
            other => return Err(Error::new("expected variant name").span(other.span())),
        };

        let data = if let Ok(TokenTree::Group(group)) = self.peek() {
            self.next()?;
            match group.delimiter() {
                Delimiter::Parenthesis => {
                    EnumVariantData::TupleLike(self.within_stream(group.stream(), |this| {
                        this.parse_comma_list(|this| this.parse_tuple_field())
                    })?)
                }
                Delimiter::Brace => {
                    EnumVariantData::StructLike(self.within_stream(group.stream(), |this| {
                        this.parse_comma_list(|this| this.parse_struct_field())
                    })?)
                }
                _ => return Err(Error::new("invalid enum variant").span(group.span())),
            }
        } else {
            EnumVariantData::None
        };

        Ok(EnumVariant { span, name, data })
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
                        ty.push_str(&self.parse_generic()?);
                    }
                }
                TokenTree::Punct(punct) if punct.as_char() == '<' => {
                    ty.push_str(&self.parse_generic()?);
                }
                other => return Err(Error::new("expected a type").span(other.span())),
            }

            if self.peek().map(|p| p.is_punct(':')).unwrap_or(false) {
                self.next()?;
                self.expect_punct(':')?;
                ty.push_str("::");
            } else {
                break;
            }
        }
        Ok(ty)
    }

    fn parse_generic(&mut self) -> Result<String, Error> {
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

    fn parse_attributes(&mut self) -> Result<Vec<Attribute>, Error> {
        let mut attributes = Vec::new();

        loop {
            if self.peek()?.is_punct('#') {
                self.next()?;
            } else {
                break;
            }

            match self.next()? {
                TokenTree::Group(group) => {
                    if group.delimiter() != Delimiter::Bracket {
                        return Err(
                            Error::new("expected braces surrounding attribute").span(group.span())
                        );
                    }
                    attributes.push(Attribute {
                        span: group.span(),
                        value: group.stream().to_string(),
                    });
                }
                other => return Err(Error::new("expected attribute").span(other.span())),
            }
        }

        Ok(attributes)
    }

    fn parse_comma_list<F, T>(&mut self, mut f: F) -> Result<Vec<T>, Error>
    where
        F: FnMut(&mut Self) -> Result<T, Error>,
    {
        let mut values = Vec::new();
        loop {
            values.push(f(self)?);
            match self.next() {
                Ok(comma) => {
                    if !comma.is_punct(',') {
                        return Err(Error::new("expected comma").span(comma.span()));
                    }
                    if self.peek().is_err() {
                        break; // End of fields list with trailing comma
                    }
                }
                Err(_) => break, // End of fields list
            }
        }
        Ok(values)
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

    fn expect_keyword(&mut self, keyword: &str) -> Result<(), Error> {
        let next = self.next()?;
        if next.is_ident(keyword) {
            Ok(())
        } else {
            Err(Error::new(format!("expected keyword {keyword}")).span(next.span()))
        }
    }

    fn expect_punct(&mut self, punct: char) -> Result<(), Error> {
        let next = self.next()?;
        if next.is_punct(punct) {
            Ok(())
        } else {
            Err(Error::new(format!("expected punctuation {punct}")).span(next.span()))
        }
    }

    fn peek(&mut self) -> Result<TokenTree, Error> {
        self.access_iter(|tokens| tokens.peek().cloned())
    }

    fn next(&mut self) -> Result<TokenTree, Error> {
        self.access_iter(|tokens| tokens.next())
    }

    fn within_braces<F, T>(&mut self, f: F) -> Result<T, Error>
    where
        F: FnOnce(&mut Self) -> Result<T, Error>,
    {
        match self.next()? {
            TokenTree::Group(group) => {
                if group.delimiter() != Delimiter::Brace {
                    return Err(Error::new("expected group delimited by braces").span(group.span()));
                }
                self.within_stream(group.stream(), |this| f(this))
            }
            other => {
                return Err(Error::new("expected group delimited by braces").span(other.span()));
            }
        }
    }

    fn within_stream<F, T>(&mut self, stream: TokenStream, f: F) -> Result<T, Error>
    where
        F: FnOnce(&mut Self) -> Result<T, Error>,
    {
        self.tokens.push(stream.into_iter().peekable());
        let result = f(self);
        self.tokens.pop();
        result
    }

    fn access_iter<F, T>(&mut self, f: F) -> Result<T, Error>
    where
        F: Fn(&mut Peekable<proc_macro::token_stream::IntoIter>) -> Option<T>,
    {
        match self.tokens.last_mut().and_then(f) {
            Some(result) => Ok(result),
            None => {
                let err = Error::new("end of input");
                Err(match self.last_span {
                    Some(span) => err.span(span),
                    None => err,
                })
            }
        }
    }
}

trait TokenTreeExt {
    fn is_ident(&self, ident: &str) -> bool;
    fn is_punct(&self, punct: char) -> bool;
}

impl TokenTreeExt for TokenTree {
    fn is_ident(&self, expected: &str) -> bool {
        match self {
            TokenTree::Ident(ident) => ident.to_string() == expected,
            _ => false,
        }
    }

    fn is_punct(&self, expected: char) -> bool {
        match self {
            TokenTree::Punct(punct) => punct.as_char() == expected,
            _ => false,
        }
    }
}
