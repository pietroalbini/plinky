use crate::lexer::{LexError, Lexer, Token};
use crate::PkgConfig;
use plinky_macros::{Display, Error};
use std::iter::Peekable;

pub(crate) struct Parser {
    tokens: Peekable<std::vec::IntoIter<Token>>,

    name: Option<String>,
    description: Option<String>,
}

impl Parser {
    pub(crate) fn new(raw: &str) -> Result<Self, ParseError> {
        Ok(Parser {
            tokens: Lexer::new(raw).lex()?.into_iter().peekable(),
            name: None,
            description: None,
        })
    }

    pub(crate) fn parse(mut self) -> Result<PkgConfig, ParseError> {
        while self.tokens.peek().is_some() {
            todo!();
        }

        Ok(PkgConfig {
            name: self.name.ok_or(ParseError::MissingField("name"))?,
            description: self.description.ok_or(ParseError::MissingField("description"))?,
        })
    }
}

#[derive(Debug, PartialEq, Eq, Error, Display)]
pub enum ParseError {
    #[transparent]
    Lex(LexError),
    #[display("missing field {f0}")]
    MissingField(&'static str),
}
