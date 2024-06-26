use crate::debug_print::filters::{ObjectsFilter, ObjectsFilterParseError};
use plinky_elf::render_elf::{RenderElfFilters, RenderElfFiltersParseError};
use plinky_macros::{Display, Error};
use std::collections::BTreeSet;
use std::path::PathBuf;

// GNU ld loves to be inconsistent, and thus some long flags are prefixed with a single dash
// rather than a double dash. To ensure we still parse the CLI correctly, we have a list of
// flags that should be emitted as LongShortFlag.
const LONG_SHORT_FLAG: &[&str] = &["no-pie", "pie"];

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct CliOptions {
    pub(crate) inputs: Vec<PathBuf>,
    pub(crate) output: PathBuf,
    pub(crate) entry: String,
    pub(crate) gc_sections: bool,
    pub(crate) debug_print: BTreeSet<DebugPrint>,
    pub(crate) executable_stack: bool,
    pub(crate) dynamic_linker: Option<String>,
    pub(crate) mode: Mode,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub(crate) enum Mode {
    PositionDependent,
    PositionIndependent,
}

#[derive(Debug, PartialEq, Eq, Clone, PartialOrd, Ord)]
pub(crate) enum DebugPrint {
    LoadedObject(ObjectsFilter),
    Gc,
    RelocatedObject(ObjectsFilter),
    Layout,
    FinalElf(RenderElfFilters),
}

pub(crate) fn parse<S: Into<String>, I: Iterator<Item = S>>(
    args: I,
) -> Result<CliOptions, CliError> {
    let args = args.map(|s| s.into()).collect::<Vec<_>>();
    let mut lexer = CliLexer::new(&args, LONG_SHORT_FLAG);

    let mut inputs = Vec::new();
    let mut output = None;
    let mut entry = None;
    let mut executable_stack = None;
    let mut gc_sections = None;
    let mut mode = None;
    let mut dynamic_linker = None;
    let mut debug_print = BTreeSet::new();

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

            CliToken::LongFlag("dynamic-linker") => {
                reject_duplicate(&token, &mut dynamic_linker, || lexer.expect_flag_value(&token))?;
            }

            CliToken::LongShortFlag("no-pie") => {
                reject_multiple_modes(&mut mode, Mode::PositionDependent)?;
            }

            CliToken::LongShortFlag("pie") => {
                reject_multiple_modes(&mut mode, Mode::PositionIndependent)?;
            }

            CliToken::ShortFlag("z") => match lexer.expect_flag_value(&token)? {
                "execstack" => reject_duplicate(
                    "-z execstack or -z noexecstack",
                    &mut executable_stack,
                    || Ok(true),
                )?,
                "noexecstack" => reject_duplicate(
                    "-z execstack or -z noexecstack",
                    &mut executable_stack,
                    || Ok(false),
                )?,
                other => return Err(CliError::UnsupportedFlag(format!("-z {other}"))),
            },

            CliToken::LongFlag("debug-print") => {
                let raw = lexer.expect_flag_value(&token)?;
                let (key, value) = raw
                    .split_once('=')
                    .map(|(key, value)| (key, Some(value)))
                    .unwrap_or((raw, None));
                let newly_inserted = debug_print.insert(match (key, value) {
                    ("loaded-object", None) => DebugPrint::LoadedObject(ObjectsFilter::all()),
                    ("loaded-object", Some(filter)) => {
                        DebugPrint::LoadedObject(ObjectsFilter::parse(filter)?)
                    }
                    ("relocated-object", None) => DebugPrint::RelocatedObject(ObjectsFilter::all()),
                    ("relocated-object", Some(filter)) => {
                        DebugPrint::RelocatedObject(ObjectsFilter::parse(filter)?)
                    }
                    ("layout", None) => DebugPrint::Layout,
                    ("final-elf", None) => DebugPrint::FinalElf(RenderElfFilters::all()),
                    ("final-elf", Some(filter)) => {
                        DebugPrint::FinalElf(RenderElfFilters::parse(filter)?)
                    }
                    ("gc", None) => DebugPrint::Gc,
                    _ => return Err(CliError::UnsupportedDebugPrint(raw.into())),
                });
                if !newly_inserted {
                    return Err(CliError::DuplicateDebugPrint(raw.into()));
                }
            }

            CliToken::LongFlag("gc-sections") => {
                reject_duplicate(&token, &mut gc_sections, || Ok(true))?
            }

            // If the flag value was not consumed in the previous iteration when the flag itself
            // was parsed, it means the flag didn't accept a value and we should error out.
            CliToken::FlagValue(_) => {
                return Err(CliError::FlagDoesNotAcceptValues(previous_token.unwrap().to_string()));
            }

            CliToken::ShortFlag(_) | CliToken::LongFlag(_) | CliToken::LongShortFlag(_) => {
                return Err(CliError::UnsupportedFlag(token.to_string()));
            }
        }
        previous_token = Some(token);
    }

    Ok(CliOptions {
        inputs,
        output: output.unwrap_or("a.out").into(),
        entry: entry.unwrap_or("_start").into(),
        gc_sections: gc_sections.unwrap_or(false),
        debug_print,
        executable_stack: executable_stack.unwrap_or(false),
        dynamic_linker: dynamic_linker.map(|s| s.into()),
        mode: mode.unwrap_or(Mode::PositionDependent),
    })
}

fn reject_duplicate<T, F: FnOnce() -> Result<T, CliError>>(
    token: impl ToString,
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

fn reject_multiple_modes(storage: &mut Option<Mode>, new: Mode) -> Result<(), CliError> {
    match storage {
        Some(_) => return Err(CliError::MultipleModeChanges),
        None => {
            *storage = Some(new);
            Ok(())
        }
    }
}

#[derive(Debug, PartialEq, Eq, Error, Display)]
pub(crate) enum CliError {
    #[display("unsupported debug print: {f0}")]
    UnsupportedDebugPrint(String),
    #[display("failed to parse debug print filter")]
    BadObjectsFilter(#[from] ObjectsFilterParseError),
    #[display("failed to parse debug print filter")]
    BadRenderElfFilter(#[from] RenderElfFiltersParseError),
    #[display("debug print enabled multiple times: {f0}")]
    DuplicateDebugPrint(String),
    #[display("flag {f0} is not supported")]
    UnsupportedFlag(String),
    #[display("flag {f0} provided multiple times")]
    DuplicateFlag(String),
    #[display("multiple flags changing the linking mode are passed")]
    MultipleModeChanges,
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

struct CliLexer<'a> {
    long_short_flags: &'static [&'static str],
    iter: std::slice::Iter<'a, String>,
    verbatim: bool,
    force_next: Option<CliToken<'a>>,
}

impl<'a> CliLexer<'a> {
    fn new(args: &'a [String], long_short_flags: &'static [&'static str]) -> Self {
        Self { long_short_flags, iter: args.iter(), verbatim: false, force_next: None }
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

    macro_rules! btreeset {
        ($($val:expr),*$(,)?) => {{
            let mut set = BTreeSet::new();
            $(set.insert($val);)*
            set
        }}
    }

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

    #[test]
    fn test_no_flags() {
        assert_eq!(
            Ok(CliOptions { inputs: Vec::new(), ..default_options() }),
            parse(std::iter::empty::<String>())
        );
    }

    #[test]
    fn test_one_input() {
        assert_eq!(
            Ok(CliOptions { inputs: vec!["foo".into()], ..default_options() }),
            parse(["foo"].into_iter())
        )
    }

    #[test]
    fn test_two_inputs() {
        assert_eq!(
            Ok(CliOptions { inputs: vec!["foo".into(), "bar".into()], ..default_options() }),
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
                parse(flags.iter().copied())
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
                parse(flags.iter().copied())
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
                parse(flags.iter().copied())
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
                parse(flags.iter().copied())
            );
        }
    }

    #[test]
    fn test_debug_print() {
        let variants = [
            (
                btreeset![DebugPrint::LoadedObject(ObjectsFilter::all())],
                &["foo", "--debug-print", "loaded-object"] as &[&str],
            ),
            (
                btreeset![DebugPrint::RelocatedObject(ObjectsFilter::all())],
                &["foo", "--debug-print", "relocated-object"],
            ),
            (
                btreeset![
                    DebugPrint::LoadedObject(ObjectsFilter::parse("@env").unwrap()),
                    DebugPrint::RelocatedObject(ObjectsFilter::all())
                ],
                &["foo", "--debug-print", "loaded-object=@env", "--debug-print=relocated-object"],
            ),
        ];
        for (expected, flags) in variants {
            assert_eq!(
                Ok(CliOptions {
                    inputs: vec!["foo".into()],
                    debug_print: expected,
                    ..default_options()
                }),
                parse(flags.iter().copied())
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
            Err(CliError::DuplicateDebugPrint("loaded-object".into())),
            parse(
                [
                    "input_file",
                    "--debug-print",
                    "relocated-object",
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
    fn test_no_executable_stack_flag() {
        let args = parse(["input_file"].into_iter()).unwrap();
        assert!(!args.executable_stack);
    }

    #[test]
    fn test_enabling_executable_stack() {
        let args = parse(["input_file", "-z", "execstack"].into_iter()).unwrap();
        assert!(args.executable_stack);
    }

    #[test]
    fn test_disabling_executable_stack() {
        let args = parse(["input_file", "-z", "noexecstack"].into_iter()).unwrap();
        assert!(!args.executable_stack);
    }

    #[test]
    fn test_multiple_executable_stack_flags() {
        let cases = [
            ["input_file", "-zexecstack", "-zexecstack"],
            ["input_file", "-znoexecstack", "-znoexecstack"],
            ["input_file", "-zexecstack", "-znoexecstack"],
        ];
        for case in cases {
            assert_eq!(
                Err(CliError::DuplicateFlag("-z execstack or -z noexecstack".into())),
                parse(case.into_iter())
            );
        }
    }

    #[test]
    fn test_gc_sections() {
        assert_eq!(
            Ok(CliOptions { inputs: vec!["foo".into()], gc_sections: true, ..default_options() }),
            parse(["foo", "--gc-sections"].into_iter())
        );
    }

    #[test]
    fn test_duplicate_gc_sections() {
        assert_eq!(
            Err(CliError::DuplicateFlag("--gc-sections".into())),
            parse(["foo", "--gc-sections", "--gc-sections"].into_iter())
        );
    }

    #[test]
    fn test_dynamic_linker() {
        assert_eq!(
            Ok(CliOptions {
                inputs: vec!["foo".into()],
                dynamic_linker: Some("bar".into()),
                ..default_options()
            }),
            parse(["foo", "--dynamic-linker=bar"].into_iter())
        );
    }

    #[test]
    fn test_duplicate_dynamic_linker() {
        assert_eq!(
            Err(CliError::DuplicateFlag("--dynamic-linker".into())),
            parse(["foo", "--dynamic-linker", "bar", "--dynamic-linker=baz"].into_iter())
        );
    }

    #[test]
    fn test_no_pie() {
        assert_eq!(
            Ok(CliOptions {
                inputs: vec!["foo".into()],
                mode: Mode::PositionDependent,
                ..default_options()
            }),
            parse(["foo", "-no-pie"].into_iter())
        );
    }

    #[test]
    fn test_pie() {
        assert_eq!(
            Ok(CliOptions {
                inputs: vec!["foo".into()],
                mode: Mode::PositionIndependent,
                ..default_options()
            }),
            parse(["foo", "-pie"].into_iter())
        );
    }

    #[test]
    fn test_duplicate_modes() {
        assert_eq!(
            Err(CliError::MultipleModeChanges),
            parse(["foo", "-no-pie", "-pie"].into_iter())
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
            gc_sections: false,
            debug_print: BTreeSet::new(),
            executable_stack: false,
            dynamic_linker: None,
            mode: Mode::PositionDependent,
        }
    }
}
