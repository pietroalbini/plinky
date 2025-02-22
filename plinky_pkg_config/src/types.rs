use crate::parser::{ParseError, Parser};

#[derive(Debug, PartialEq, Eq)]
pub struct PkgConfig {
    pub name: Option<String>,
    pub description: Option<String>,
    pub url: Option<String>,
    pub version: Option<String>,
    pub requires: Option<String>,
    pub requires_private: Option<String>,
    pub conflicts: Option<String>,
    pub cflags: Option<String>,
    pub libs: Option<String>,
    pub libs_private: Option<String>,
}

impl PkgConfig {
    pub fn parse(content: &str) -> Result<Self, ParseError> {
        Parser::new(content)?.parse()
    }
}
