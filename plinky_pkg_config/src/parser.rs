use crate::lexer::{is_valid_identifier, LexError, Lexer, Token};
use crate::template::{resolve_variables, Resolvable, Template, TemplateComponent};
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
    cflags: Option<Vec<Template>>,
    libs: Option<Vec<Template>>,
    libs_private: Option<Vec<Template>>,
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
                            f.resolve($resolved, &WhileResolving::Field(stringify!($field)))
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
                let field: Field = match key.as_str() {
                    "Name" => StringField::Name.into(),
                    "Description" => StringField::Description.into(),
                    "URL" => StringField::Url.into(),
                    "Version" => StringField::Version.into(),
                    "Requires" => StringField::Requires.into(),
                    "Requires.private" => StringField::RequiresPrivate.into(),
                    "Conflicts" => StringField::Conflicts.into(),
                    "Cflags" => ArgsField::CFlags.into(),
                    "Libs" => ArgsField::Libs.into(),
                    "Libs.private" => ArgsField::LibsPrivate.into(),
                    _ => return Err(ParseError::UnknownField(key)),
                };

                match field {
                    Field::String(field) => {
                        self.skip_whitespace();
                        let value = self.parse_template()?;

                        let storage = match field {
                            StringField::Name => &mut self.name,
                            StringField::Description => &mut self.description,
                            StringField::Url => &mut self.url,
                            StringField::Version => &mut self.version,
                            StringField::Requires => &mut self.requires,
                            StringField::RequiresPrivate => &mut self.requires_private,
                            StringField::Conflicts => &mut self.conflicts,
                        };
                        if storage.is_some() {
                            return Err(ParseError::DuplicateField(key));
                        }
                        *storage = Some(value);
                    }
                    Field::Args(field) => {
                        self.skip_whitespace();
                        let value = self.parse_args()?;

                        let storage = match field {
                            ArgsField::CFlags => &mut self.cflags,
                            ArgsField::Libs => &mut self.libs,
                            ArgsField::LibsPrivate => &mut self.libs_private,
                        };
                        if storage.is_some() {
                            return Err(ParseError::DuplicateField(key));
                        }
                        *storage = Some(value);
                    }
                }
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
        let mut template = Template::new();
        let mut pending_whitespace = String::new();

        while self.tokens.peek() != Some(&Token::NewLine) {
            if !pending_whitespace.is_empty() {
                template.push(TemplateComponent::Text(take(&mut pending_whitespace)));
            }

            template.push(match self.next_token() {
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

    fn parse_args(&mut self) -> Result<Vec<Template>, ParseError> {
        let mut result = Vec::new();
        let mut current = Template::new();
        let mut quote = QuoteMode::None;

        // TODO: implement backslash to escape

        loop {
            match self.next_token() {
                Some(Token::SingleQuote) => match quote {
                    QuoteMode::None => quote = QuoteMode::Single,
                    QuoteMode::Single => quote = QuoteMode::None,
                    QuoteMode::Double => current.push(TemplateComponent::TextStatic("'")),
                },
                Some(Token::DoubleQuote) => match quote {
                    QuoteMode::None => quote = QuoteMode::Double,
                    QuoteMode::Double => quote = QuoteMode::None,
                    QuoteMode::Single => current.push(TemplateComponent::TextStatic("\"")),
                },
                Some(Token::Backslash) => current.push(TemplateComponent::TextStatic("\\")),

                Some(Token::Colon) => current.push(TemplateComponent::TextStatic(":")),
                Some(Token::Equals) => current.push(TemplateComponent::TextStatic("=")),

                Some(Token::Text(text)) => current.push(TemplateComponent::Text(text)),
                Some(Token::Variable(var)) => current.push(TemplateComponent::Variable(var)),
                Some(Token::Whitespace(whitespace)) => {
                    if quote.is_quoted() {
                        current.push(TemplateComponent::Text(whitespace));
                    } else {
                        if !current.is_empty() {
                            result.push(take(&mut current));
                        }
                    }
                }

                Some(Token::NewLine) => break,
                None => break, // EOF
            }
        }

        if quote.is_quoted() {
            return Err(ParseError::UnterminatedQuote);
        }

        if !current.is_empty() {
            result.push(current);
        }
        Ok(result)
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

enum QuoteMode {
    None,
    Single,
    Double,
}

impl QuoteMode {
    fn is_quoted(&self) -> bool {
        match self {
            QuoteMode::None => false,
            QuoteMode::Single => true,
            QuoteMode::Double => true,
        }
    }
}

enum Field {
    String(StringField),
    Args(ArgsField),
}

impl From<StringField> for Field {
    fn from(value: StringField) -> Self {
        Field::String(value)
    }
}

impl From<ArgsField> for Field {
    fn from(value: ArgsField) -> Self {
        Field::Args(value)
    }
}

enum StringField {
    Name,
    Description,
    Url,
    Version,
    Requires,
    RequiresPrivate,
    Conflicts,
}

enum ArgsField {
    CFlags,
    Libs,
    LibsPrivate,
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
    #[display("unterminated quote")]
    UnterminatedQuote,
}
