use crate::lexer::{is_valid_identifier, LexError, Lexer, Token};
use crate::template::{resolve_variables, Template, TemplateComponent};
use crate::PkgConfig;
use plinky_macros::{Display, Error};
use std::collections::BTreeMap;
use std::iter::Peekable;
use std::mem::take;

pub(crate) struct Parser {
    tokens: Peekable<std::vec::IntoIter<Token>>,
    variables: BTreeMap<String, Template>,

    name: Option<Template>,
    description: Option<Template>,
    url: Option<Template>,
    version: Option<Template>,
    requires: Option<Template>,
    requires_private: Option<Template>,
    conflicts: Option<Template>,
    cflags: Option<Template>,
    libs: Option<Template>,
    libs_private: Option<Template>,
}

impl Parser {
    pub(crate) fn new(raw: &str) -> Result<Self, ParseError> {
        Ok(Parser {
            tokens: Lexer::new(raw).lex()?.into_iter().peekable(),
            variables: BTreeMap::new(),
            name: None,
            description: None,
            url: None,
            version: None,
            requires: None,
            requires_private: None,
            conflicts: None,
            cflags: None,
            libs: None,
            libs_private: None,
        })
    }

    pub(crate) fn parse(mut self) -> Result<PkgConfig, ParseError> {
        while self.tokens.peek().is_some() {
            self.parse_line()?;
        }

        macro_rules! resolve {
            ($resolved:expr, $ty:ident { $($field:ident,)* }) => {
                $ty {
                    $(
                        $field: self.$field.map(|f|
                            f.resolve($resolved, WhileResolving::Field(stringify!($field)))
                        ).transpose()?,
                    )*
                }
            }
        }
        let vars = resolve_variables(&mut self.variables)?;
        Ok(resolve!(
            &vars,
            PkgConfig {
                name,
                description,
                url,
                version,
                requires,
                requires_private,
                conflicts,
                cflags,
                libs,
                libs_private,
            }
        ))
    }

    fn parse_line(&mut self) -> Result<(), ParseError> {
        self.skip_whitespace();

        let key = match self.next_token() {
            Some(Token::NewLine) | None => return Ok(()),
            Some(Token::Text(key)) => key,
            other => return self.unexpected(other, "directive or variable name"),
        };

        self.skip_whitespace();

        match self.next_token() {
            Some(Token::Colon) => {
                self.skip_whitespace();
                let value = self.parse_template()?;

                let storage = match key.as_str() {
                    "Name" => &mut self.name,
                    "Description" => &mut self.description,
                    "URL" => &mut self.url,
                    "Version" => &mut self.version,
                    "Requires" => &mut self.requires,
                    "Requires.private" => &mut self.requires_private,
                    "Conflicts" => &mut self.conflicts,
                    "Cflags" => &mut self.cflags,
                    "Libs" => &mut self.libs,
                    "Libs.private" => &mut self.libs_private,
                    _ => return Err(ParseError::UnknownField(key)),
                };
                if storage.is_some() {
                    return Err(ParseError::DuplicateField(key));
                }
                *storage = Some(value);
            }
            Some(Token::Equals) => {
                self.skip_whitespace();
                let value = self.parse_template()?;

                if !key.chars().all(is_valid_identifier) {
                    return Err(ParseError::InvalidVariableName(key));
                }
                if self.variables.contains_key(&key) {
                    return Err(ParseError::DuplicateVariable(key));
                }
                self.variables.insert(key, value);
            }
            other => return self.unexpected(other, ": or ="),
        }

        self.skip_whitespace();
        match self.next_token() {
            Some(Token::NewLine) | None => {}
            other => return self.unexpected(other, "newline"),
        }

        Ok(())
    }

    fn parse_template(&mut self) -> Result<Template, ParseError> {
        let mut template = Template { components: Vec::new() };
        let mut pending_whitespace = String::new();

        while self.tokens.peek() != Some(&Token::NewLine) {
            if !pending_whitespace.is_empty() {
                template.components.push(TemplateComponent::Text(take(&mut pending_whitespace)));
            }

            template.components.push(match self.next_token() {
                Some(Token::Colon) => TemplateComponent::TextStatic(":"),
                Some(Token::Equals) => TemplateComponent::TextStatic("="),
                Some(Token::SingleQuote) => TemplateComponent::TextStatic("'"),
                Some(Token::DoubleQuote) => TemplateComponent::TextStatic("\""),
                Some(Token::Backslash) => TemplateComponent::TextStatic("\\"),
                Some(Token::Text(text)) => TemplateComponent::Text(text),
                Some(Token::Variable(var)) => TemplateComponent::Variable(var),
                Some(Token::Whitespace(whitespace)) => {
                    pending_whitespace.push_str(&whitespace);
                    continue;
                }
                Some(Token::NewLine) => unreachable!(),
                None => break, // EOF
            });
        }

        Ok(template)
    }

    fn skip_whitespace(&mut self) {
        while let Some(Token::Whitespace(_)) = self.tokens.peek() {
            self.next_token();
        }
    }

    fn unexpected<T>(&self, got: Option<Token>, expected: &'static str) -> Result<T, ParseError> {
        match got {
            Some(token) => Err(ParseError::Unexpected(expected, format!("{token}"))),
            None => Err(ParseError::UnexpectedEof(expected)),
        }
    }

    fn next_token(&mut self) -> Option<Token> {
        self.tokens.next()
    }
}

#[derive(Debug, PartialEq, Eq, Display, Clone)]
pub enum WhileResolving {
    #[display("field {f0}")]
    Field(&'static str),
    #[display("variable {f0}")]
    Variable(String),
}

#[derive(Debug, PartialEq, Eq, Error, Display)]
pub enum ParseError {
    #[transparent]
    Lex(LexError),
    #[display("expected {f0}, found {f1}")]
    Unexpected(&'static str, String),
    #[display("expected {f0}, but input ended")]
    UnexpectedEof(&'static str),
    #[display("invalid variable name: {f0}")]
    InvalidVariableName(String),
    #[display("duplicate variable: {f0}")]
    DuplicateVariable(String),
    #[display("undefined variable while resolving {f1}: {f0}")]
    UndefinedVariable(String, WhileResolving),
    #[display("expanded content is too large (while resolving {f0})")]
    ContentTooLarge(WhileResolving),
    #[display("unknown field {f0}")]
    UnknownField(String),
    #[display("duplicate field {f0}")]
    DuplicateField(String),
}
