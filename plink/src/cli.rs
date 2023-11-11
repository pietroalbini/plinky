use plink_macros::{Error, Display};
use std::path::PathBuf;

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct CliOptions {
    pub(crate) inputs: Vec<PathBuf>,
    pub(crate) output: PathBuf,
    pub(crate) entry: String,
    pub(crate) debug_print: Option<DebugPrint>,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub(crate) enum DebugPrint {
    LoadedObject,
    RelocatedObject,
    Layout,
    FinalElf,
}

pub(crate) fn parse<S: Into<String>, I: Iterator<Item = S>>(
    args: I,
) -> Result<CliOptions, CliError> {
    let args = args.map(|s| s.into()).collect::<Vec<_>>();
    let mut lexer = CliLexer::new(&args);

    let mut inputs = Vec::new();
    let mut output = None;
    let mut entry = None;
    let mut debug_print = None;

    let mut previous_token: Option<CliToken<'_>> = None;
    while let Some(token) = lexer.next() {
        match token {
            CliToken::StandaloneValue(val) => inputs.push(val.into()),

            CliToken::LongFlag("output") | CliToken::ShortFlag("o") => {
                reject_duplicate(&token, &mut output, || lexer.expect_flag_value(&token))?;
            }

            CliToken::LongFlag("entry") | CliToken::ShortFlag("e") => {
                reject_duplicate(&token, &mut entry, || lexer.expect_flag_value(&token))?;
            }

            CliToken::LongFlag("debug-print") => {
                reject_duplicate(&token, &mut debug_print, || {
                    Ok(match lexer.expect_flag_value(&token)? {
                        "loaded-object" => DebugPrint::LoadedObject,
                        "relocated-object" => DebugPrint::RelocatedObject,
                        "layout" => DebugPrint::Layout,
                        "final-elf" => DebugPrint::FinalElf,
                        other => return Err(CliError::UnsupportedDebugPrint(other.into())),
                    })
                })?;
            }

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

    if inputs.is_empty() {
        return Err(CliError::MissingInput);
    }

    Ok(CliOptions {
        inputs,
        output: output.unwrap_or("a.out").into(),
        entry: entry.unwrap_or("_start").into(),
        debug_print,
    })
}

fn reject_duplicate<T, F: FnOnce() -> Result<T, CliError>>(
    token: &CliToken<'_>,
    storage: &mut Option<T>,
    f: F,
) -> Result<(), CliError> {
    match storage {
        Some(_) => Err(CliError::DuplicateFlag(token.to_string())),
        None => {
            *storage = Some(f()?);
            Ok(())
        }
    }
}

#[derive(Debug, PartialEq, Eq, Error, Display)]
pub(crate) enum CliError {
    #[display("missing input file")]
    MissingInput,
    #[display("unsupported debug print: {f0}")]
    UnsupportedDebugPrint(String),
    #[display("flag {f0} is not supported")]
    UnsupportedFlag(String),
    #[display("flag {f0} provided multiple times")]
    DuplicateFlag(String),
    #[display("flag {f0} does not accept values")]
    FlagDoesNotAcceptValues(String),
    #[display("missing value for flag {f0}")]
    MissingValueForFlag(String),
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
                inputs: vec!["foo".into()],
                ..default_options()
            }),
            parse(["foo"].into_iter())
        )
    }

    #[test]
    fn test_two_inputs() {
        assert_eq!(
            Ok(CliOptions {
                inputs: vec!["foo".into(), "bar".into()],
                ..default_options()
            }),
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
                    inputs: vec!["foo".into()],
                    output: "bar".into(),
                    ..default_options()
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
    fn test_entry_flags() {
        const VARIANTS: &[&[&str]] = &[
            &["foo", "-ebar"],
            &["foo", "-e", "bar"],
            &["foo", "--entry=bar"],
            &["foo", "--entry", "bar"],
        ];

        for flags in VARIANTS {
            assert_eq!(
                Ok(CliOptions {
                    inputs: vec!["foo".into()],
                    entry: "bar".into(),
                    ..default_options()
                }),
                parse(flags.into_iter().copied())
            );
        }
    }

    #[test]
    fn test_multiple_entry_flags() {
        const VARIANTS: &[(&str, &[&str])] = &[
            ("-e", &["foo", "-ebar", "-ebaz"]),
            ("-e", &["foo", "-e", "bar", "-e", "baz"]),
            ("--entry", &["foo", "--entry=bar", "--entry=baz"]),
            ("--entry", &["foo", "--entry", "bar", "--entry", "baz"]),
        ];

        for (duplicate, flags) in VARIANTS {
            assert_eq!(
                Err(CliError::DuplicateFlag((*duplicate).into())),
                parse(flags.into_iter().copied())
            );
        }
    }

    #[test]
    fn test_debug_print() {
        const VARIANTS: &[(DebugPrint, &[&str])] = &[(
            DebugPrint::LoadedObject,
            &["foo", "--debug-print", "loaded-object"],
        )];
        for (expected, flags) in VARIANTS {
            assert_eq!(
                Ok(CliOptions {
                    inputs: vec!["foo".into()],
                    debug_print: Some(*expected),
                    ..default_options()
                }),
                parse(flags.into_iter().copied())
            )
        }
    }

    #[test]
    fn test_unsupported_debug_print() {
        assert_eq!(
            Err(CliError::UnsupportedDebugPrint("foo".into())),
            parse(["input_file", "--debug-print", "foo"].into_iter())
        );
    }

    #[test]
    fn test_duplicate_debug_print() {
        assert_eq!(
            Err(CliError::DuplicateFlag("--debug-print".into())),
            parse(
                [
                    "input_file",
                    "--debug-print",
                    "loaded-object",
                    "--debug-print",
                    "loaded-object"
                ]
                .into_iter()
            )
        );
    }

    #[test]
    fn test_unknown_flags() {
        assert_eq!(
            Err(CliError::UnsupportedFlag("--foo-bar".into())),
            parse(["--foo-bar"].into_iter())
        )
    }

    fn default_options() -> CliOptions {
        CliOptions {
            inputs: Vec::new(),
            output: "a.out".into(),
            entry: "_start".into(),
            debug_print: None,
        }
    }
}
