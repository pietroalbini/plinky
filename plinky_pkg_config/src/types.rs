use crate::parser::{ParseError, Parser};

pub struct PkgConfig {
    pub name: String,
    pub description: String,
}

impl PkgConfig {
    pub fn parse(content: &str) -> Result<Self, ParseError> {
        Parser::new(content)?.parse()
    }
}
