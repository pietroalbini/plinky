use crate::error::Error;
use crate::parser::Parser;
use proc_macro::{Delimiter, TokenStream, TokenTree};
use std::iter::Peekable;

impl Parser {
    pub(super) fn parse_comma_list<F, T>(&mut self, mut f: F) -> Result<Vec<T>, Error>
    where
        F: FnMut(&mut Self) -> Result<IterationOutcome<T>, Error>,
    {
        let mut values = Vec::new();
        loop {
            match f(self)? {
                IterationOutcome::Value(v) => values.push(v),
                IterationOutcome::Last(v) => {
                    values.push(v);
                    break;
                }
                IterationOutcome::Break => break,
            }
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

    pub(super) fn expect_keyword(&mut self, keyword: &str) -> Result<(), Error> {
        let next = self.next()?;
        if next.is_ident(keyword) {
            Ok(())
        } else {
            Err(Error::new(format!("expected keyword {keyword}")).span(next.span()))
        }
    }

    pub(super) fn expect_punct(&mut self, punct: char) -> Result<(), Error> {
        let next = self.next()?;
        if next.is_punct(punct) {
            Ok(())
        } else {
            Err(Error::new(format!("expected punctuation {punct}")).span(next.span()))
        }
    }

    pub(super) fn peek(&mut self) -> Result<TokenTree, Error> {
        self.access_iter(|tokens| tokens.peek().cloned())
    }

    pub(super) fn next(&mut self) -> Result<TokenTree, Error> {
        self.access_iter(|tokens| tokens.next())
    }

    pub(super) fn within_braces<F, T>(&mut self, f: F) -> Result<T, Error>
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
            other => Err(Error::new("expected group delimited by braces").span(other.span())),
        }
    }

    pub(super) fn within_stream<F, T>(&mut self, stream: TokenStream, f: F) -> Result<T, Error>
    where
        F: FnOnce(&mut Self) -> Result<T, Error>,
    {
        self.tokens.push(stream.into_iter().peekable());
        let result = f(self);
        self.tokens.pop();
        result
    }

    pub(super) fn access_iter<F, T>(&mut self, f: F) -> Result<T, Error>
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

pub(super) trait TokenTreeExt {
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

pub(super) enum IterationOutcome<T> {
    Value(T),
    Last(T),
    Break,
}

impl<T> From<T> for IterationOutcome<T> {
    fn from(value: T) -> Self {
        IterationOutcome::Value(value)
    }
}
