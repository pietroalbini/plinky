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
    pub cflags: Option<Vec<String>>,
    pub libs: Option<Vec<String>>,
    pub libs_private: Option<Vec<String>>,
}

impl PkgConfig {
    pub fn parse(content: &str) -> Result<Self, ParseError> {
        Parser::new(content)?.parse()
    }
}
