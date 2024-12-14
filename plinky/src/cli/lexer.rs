use crate::cli::CliError;

#[derive(Debug, PartialEq, Eq)]
pub(super) enum CliToken<'a> {
    StandaloneValue(&'a str),
    FlagValue(&'a str),
    ShortFlag(&'a str),
    LongFlag(&'a str),
    LongShortFlag(&'a str),
}

impl std::fmt::Display for CliToken<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CliToken::StandaloneValue(v) | CliToken::FlagValue(v) => f.write_str(v),
            CliToken::ShortFlag(flag) => write!(f, "-{flag}"),
            CliToken::LongFlag(flag) => write!(f, "--{flag}"),
            CliToken::LongShortFlag(flag) => write!(f, "-{flag}"),
        }
    }
}

pub(super) struct CliLexer<'a> {
    long_short_flags: &'static [&'static str],
    iter: std::slice::Iter<'a, String>,
    verbatim: bool,
    force_next: Option<CliToken<'a>>,
}

impl<'a> CliLexer<'a> {
    pub(super) fn new(args: &'a [String], long_short_flags: &'static [&'static str]) -> Self {
        Self { long_short_flags, iter: args.iter(), verbatim: false, force_next: None }
    }

    pub(super) fn expect_flag_value(&mut self, flag: &CliToken<'_>) -> Result<&'a str, CliError> {
        match self.next() {
            Some(CliToken::FlagValue(value)) if !value.is_empty() => Ok(value),
            Some(CliToken::StandaloneValue(value)) if !value.is_empty() => Ok(value),
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

            if let Some(option) = token.strip_prefix('-') {
                // Handle long flags starting with a single dash, sigh.
                for long_short_flag in self.long_short_flags {
                    if option == *long_short_flag {
                        return Some(CliToken::LongShortFlag(option));
                    } else if let Some(value) =
                        option.strip_prefix(long_short_flag).and_then(|o| o.strip_prefix('='))
                    {
                        self.force_next = Some(CliToken::FlagValue(value));
                        return Some(CliToken::LongShortFlag(*long_short_flag));
                    }
                }

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

        let long_short_flags = &["xy"];
        let args = &[
            "a", "b", "--c", "d", "--e=f", "-", "-g", "h", "-ijkl", "-xy", "-xy=z", "-xyz", "--",
            "-mo", "--pq", "--", "r",
        ];
        let args_str = args.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let tokens = CliLexer::new(&args_str, long_short_flags).collect::<Vec<_>>();

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
                LongShortFlag("xy"),
                LongShortFlag("xy"),
                FlagValue("z"),
                ShortFlag("x"),
                FlagValue("yz"),
                StandaloneValue("-mo"),
                StandaloneValue("--pq"),
                StandaloneValue("--"),
                StandaloneValue("r"),
            ],
            tokens.as_slice()
        );
    }
}
