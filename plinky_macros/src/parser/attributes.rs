use crate::error::Error;
use crate::parser::utils::{IterationOutcome, TokenTreeExt};
use crate::parser::{Attribute, AttributeContent, AttributeValue, Attributes, Parser};
use proc_macro::{Delimiter, Span, TokenStream, TokenTree};

impl Parser {
    pub(super) fn parse_attributes(&mut self) -> Result<Attributes, Error> {
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
                    attributes.push(self.within_stream(group.stream(), |this| {
                        this.parse_attribute(group.span())
                    })?);
                }
                other => return Err(Error::new("expected attribute").span(other.span())),
            }
        }

        Ok(Attributes { attributes })
    }

    fn parse_attribute(&mut self, span: Span) -> Result<Attribute, Error> {
        let name = match self.next()? {
            TokenTree::Ident(ident) => ident.to_string(),
            other => return Err(Error::new("expected attribute name").span(other.span())),
        };

        let content = if self.is_end_of_input() {
            AttributeContent::Empty
        } else {
            match self.next()? {
                token if token.is_punct('=') => {
                    let value = self.parse_attribute_value()?;
                    if !self.is_end_of_input() {
                        return Err(
                            Error::new("attribute with = contains more than one value").span(span)
                        );
                    }
                    AttributeContent::EqualsTo(value)
                }
                TokenTree::Group(group) if group.delimiter() == Delimiter::Parenthesis => {
                    AttributeContent::ParenthesisList(self.within_stream(
                        group.stream(),
                        |this| {
                            this.parse_comma_list(|this| {
                                Ok(IterationOutcome::Value(this.parse_attribute_value()?))
                            })
                        },
                    )?)
                }
                _ => return Err(Error::new("unsupported attribute syntax").span(span)),
            }
        };

        Ok(Attribute { span, name, content })
    }

    fn parse_attribute_value(&mut self) -> Result<AttributeValue, Error> {
        Ok(match self.next()? {
            TokenTree::Literal(literal) => {
                let string = literal.to_string();
                if let Some(string) = string.strip_prefix('"').and_then(|l| l.strip_suffix('"')) {
                    AttributeValue::String(string.to_string())
                } else {
                    AttributeValue::Expr(TokenStream::from(TokenTree::Literal(literal)))
                }
            }
            other => AttributeValue::Expr(TokenStream::from(other)),
        })
    }
}
