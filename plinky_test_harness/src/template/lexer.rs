use crate::template::TemplateParseError;

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Token<'a> {
    RawText(&'a str),
    Variable(&'a str),
    BeginInterpolation,
    EndInterpolation,
}

impl std::fmt::Display for Token<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Token::RawText(_) => f.write_str("raw text"),
            Token::Variable(var) => f.write_str(var),
            Token::BeginInterpolation => f.write_str("${"),
            Token::EndInterpolation => f.write_str("}"),
        }
    }
}

pub(super) struct Lexer<'a, 'b> {
    remaining: Option<&'b mut &'a str>,
    inside_interpolation: bool,
}

impl<'a, 'b> Lexer<'a, 'b> {
    pub(super) fn new(input: &'b mut &'a str) -> Self {
        Self { remaining: Some(input), inside_interpolation: false }
    }
}

impl<'a> Iterator for Lexer<'a, '_> {
    type Item = Result<Token<'a>, TemplateParseError>;

    fn next(&mut self) -> Option<Self::Item> {
        let remaining = self.remaining.as_mut()?;
        if self.inside_interpolation {
            loop {
                return match remaining.chars().next() {
                    Some('}') => {
                        self.inside_interpolation = false;
                        **remaining = &remaining[1..];
                        Some(Ok(Token::EndInterpolation))
                    }
                    Some('a'..='z' | 'A'..='Z') => {
                        let end = remaining
                            .char_indices()
                            .filter(|(_idx, chr)| !matches!(chr, 'a'..='z' | 'A'..='Z' | '.'))
                            .map(|(idx, _chr)| idx)
                            .next()
                            .unwrap_or(remaining.len());
                        let result = Token::Variable(&remaining[..end]);
                        **remaining = &remaining[end..];
                        Some(Ok(result))
                    }
                    Some(' ' | '\t') => {
                        **remaining = &remaining[1..];
                        continue;
                    },
                    Some(chr) => Some(Err(TemplateParseError::UnexpectedChar(chr))),
                    None => None,
                };
            }
        } else {
            match remaining.find("${") {
                Some(0) => {
                    self.inside_interpolation = true;
                    **remaining = &remaining[2..];
                    Some(Ok(Token::BeginInterpolation))
                }
                Some(end) => {
                    let result = Token::RawText(&remaining[..end]);
                    **remaining = &remaining[end..];
                    Some(Ok(result))
                }
                None => {
                    let result = Token::RawText(&*remaining);
                    self.remaining = None;
                    Some(Ok(result))
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lexer_good() {
        assert_eq!(
            vec![
                Token::RawText("Hello "),
                Token::BeginInterpolation,
                Token::Variable("user"),
                Token::EndInterpolation,
                Token::RawText("! I am "),
                Token::BeginInterpolation,
                Token::Variable("caller.role"),
                Token::Variable("caller.name.both")
            ],
            Lexer::new(&mut "Hello ${user}! I am ${   caller.role \t caller.name.both")
                .map(|t| t.unwrap())
                .collect::<Vec<_>>(),
        );
    }
}
