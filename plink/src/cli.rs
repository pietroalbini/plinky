use std::path::PathBuf;

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct CliOptions {
    pub(crate) input: PathBuf,
    pub(crate) output: PathBuf,
}

pub(crate) fn parse<S: Into<String>, I: Iterator<Item = S>>(
    args: I,
) -> Result<CliOptions, CliError> {
    let args = args.map(|s| s.into()).collect::<Vec<_>>();
    let mut lexer = CliLexer::new(&args);

    let mut input = None;
    let mut output = None;

    let mut previous_token: Option<CliToken<'_>> = None;
    while let Some(token) = lexer.next() {
        match token {
            CliToken::StandaloneValue(val) => match input {
                Some(_) => return Err(CliError::MultipleInputs),
                None => input = Some(val.to_string()),
            },

            CliToken::LongFlag("output") | CliToken::ShortFlag("o") => match output {
                Some(_) => return Err(CliError::DuplicateFlag(token.to_string())),
                None => output = Some(lexer.expect_flag_value(&token)?),
            },

            // If the flag value was not consumed in the previous iteration when the flag itself
            // was parsed, it means the flag didn't accept a value and we should error out.
            CliToken::FlagValue(_) => {
                return Err(CliError::FlagDoesNotAcceptValues(
                    previous_token.unwrap().to_string(),
                ));
            }

            CliToken::ShortFlag(_) | CliToken::LongFlag(_) => {
                return Err(CliError::UnsupportedFlag(token.to_string()));
            }
        }
        previous_token = Some(token);
    }

    Ok(CliOptions {
        input: input.ok_or(CliError::MissingInput)?.into(),
        output: output.unwrap_or("a.out").into(),
    })
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum CliError {
    MissingInput,
    MultipleInputs,
    UnsupportedFlag(String),
    DuplicateFlag(String),
    FlagDoesNotAcceptValues(String),
    MissingValueForFlag(String),
}

impl std::error::Error for CliError {}

impl std::fmt::Display for CliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CliError::MissingInput => f.write_str("missing input file"),
            CliError::MultipleInputs => f.write_str("multiple input files are not yet supported"),
            CliError::UnsupportedFlag(flag) => write!(f, "flag {flag} is not supported"),
            CliError::DuplicateFlag(flag) => write!(f, "flag {flag} provided multiple times"),
            CliError::FlagDoesNotAcceptValues(flag) => {
                write!(f, "flag {flag} does not accept values")
            }
            CliError::MissingValueForFlag(flag) => write!(f, "missing value for flag {flag}"),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum CliToken<'a> {
    StandaloneValue(&'a str),
    FlagValue(&'a str),
    ShortFlag(&'a str),
    LongFlag(&'a str),
}

impl std::fmt::Display for CliToken<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CliToken::StandaloneValue(v) | CliToken::FlagValue(v) => f.write_str(*v),
            CliToken::ShortFlag(flag) => write!(f, "-{flag}"),
            CliToken::LongFlag(flag) => write!(f, "--{flag}"),
        }
    }
}

struct CliLexer<'a> {
    iter: std::slice::Iter<'a, String>,
    verbatim: bool,
    force_next: Option<CliToken<'a>>,
}

impl<'a> CliLexer<'a> {
    fn new(args: &'a [String]) -> Self {
        Self {
            iter: args.iter(),
            verbatim: false,
            force_next: None,
        }
    }

    fn expect_flag_value(&mut self, flag: &CliToken<'_>) -> Result<&'a str, CliError> {
        match self.next() {
            Some(CliToken::FlagValue(value)) | Some(CliToken::StandaloneValue(value)) => Ok(value),
            _ => Err(CliError::MissingValueForFlag(flag.to_string())),
        }
    }
}

impl<'a> Iterator for CliLexer<'a> {
    type Item = CliToken<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(next) = self.force_next.take() {
            return Some(next);
        }
        loop {
            let token = self.iter.next()?;
            if self.verbatim || token == "-" {
                return Some(CliToken::StandaloneValue(token));
            }

            if token == "--" {
                self.verbatim = true;
                continue;
            }

            if let Some(option) = token.strip_prefix("--") {
                match option.split_once('=') {
                    Some((option, value)) => {
                        self.force_next = Some(CliToken::FlagValue(value));
                        return Some(CliToken::LongFlag(option));
                    }
                    None => return Some(CliToken::LongFlag(option)),
                }
            }

            if let Some(option) = token.strip_prefix("-") {
                if option.len() == 1 {
                    return Some(CliToken::ShortFlag(option));
                } else {
                    let (option, value) = option.split_at(1);
                    self.force_next = Some(CliToken::FlagValue(value));
                    return Some(CliToken::ShortFlag(option));
                }
            }

            return Some(CliToken::StandaloneValue(token));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lexer() {
        use CliToken::*;

        let args = &[
            "a", "b", "--c", "d", "--e=f", "-", "-g", "h", "-ijkl", "--", "-mo", "--pq", "--", "r",
        ];
        let args_str = args.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let tokens = CliLexer::new(&args_str).collect::<Vec<_>>();

        assert_eq!(
            &[
                StandaloneValue("a"),
                StandaloneValue("b"),
                LongFlag("c"),
                StandaloneValue("d"),
                LongFlag("e"),
                FlagValue("f"),
                StandaloneValue("-"),
                ShortFlag("g"),
                StandaloneValue("h"),
                ShortFlag("i"),
                FlagValue("jkl"),
                StandaloneValue("-mo"),
                StandaloneValue("--pq"),
                StandaloneValue("--"),
                StandaloneValue("r"),
            ],
            tokens.as_slice()
        );
    }

    #[test]
    fn test_no_flags() {
        assert_eq!(
            Err(CliError::MissingInput),
            parse(std::iter::empty::<String>())
        );
    }

    #[test]
    fn test_one_input() {
        assert_eq!(
            Ok(CliOptions {
                input: "foo".into(),
                output: "a.out".into()
            }),
            parse(["foo"].into_iter())
        )
    }

    #[test]
    fn test_two_inputs() {
        assert_eq!(
            Err(CliError::MultipleInputs),
            parse(["foo", "bar"].into_iter())
        )
    }

    #[test]
    fn test_output_flags() {
        const VARIANTS: &[&[&str]] = &[
            &["foo", "-obar"],
            &["foo", "-o", "bar"],
            &["foo", "--output=bar"],
            &["foo", "--output", "bar"],
        ];

        for flags in VARIANTS {
            assert_eq!(
                Ok(CliOptions {
                    input: "foo".into(),
                    output: "bar".into(),
                }),
                parse(flags.into_iter().copied())
            );
        }
    }

    #[test]
    fn test_multiple_output_flags() {
        const VARIANTS: &[(&str, &[&str])] = &[
            ("-o", &["foo", "-obar", "-obaz"]),
            ("-o", &["foo", "-o", "bar", "-o", "baz"]),
            ("--output", &["foo", "--output=bar", "--output=baz"]),
            ("--output", &["foo", "--output", "bar", "--output", "baz"]),
        ];

        for (duplicate, flags) in VARIANTS {
            assert_eq!(
                Err(CliError::DuplicateFlag((*duplicate).into())),
                parse(flags.into_iter().copied())
            );
        }
    }

    #[test]
    fn test_unknown_flags() {
        assert_eq!(
            Err(CliError::UnsupportedFlag("--foo-bar".into())),
            parse(["--foo-bar"].into_iter())
        )
    }
}
