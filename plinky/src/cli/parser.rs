use crate::cli::lexer::{CliLexer, CliToken};
use crate::cli::{CliError, CliInput, CliOptions, DebugPrint, DynamicLinker, Mode};
use crate::debug_print::filters::ObjectsFilter;
use plinky_elf::render_elf::RenderElfFilters;
use std::collections::BTreeSet;
use std::path::PathBuf;

// GNU ld loves to be inconsistent, and thus some long flags are prefixed with a single dash
// rather than a double dash. To ensure we still parse the CLI correctly, we have a list of
// flags that should be emitted as LongShortFlag.
const LONG_SHORT_FLAG: &[&str] = &["no-pie", "pie", "shared", "soname"];

pub(crate) fn parse<S: Into<String>, I: Iterator<Item = S>>(
    args: I,
) -> Result<CliOptions, CliError> {
    let args = args.map(|s| s.into()).collect::<Vec<_>>();
    let mut lexer = CliLexer::new(&args, LONG_SHORT_FLAG);

    let mut inputs = Vec::new();
    let mut output = None;
    let mut entry = None;
    let mut executable_stack = None;
    let mut read_only_got = None;
    let mut read_only_got_plt = None;
    let mut gc_sections = None;
    let mut mode = None;
    let mut dynamic_linker = None;
    let mut search_paths = Vec::new();
    let mut shared_object_name = None;
    let mut debug_print = BTreeSet::new();

    let mut previous_token: Option<CliToken<'_>> = None;
    while let Some(token) = lexer.next() {
        match token {
            CliToken::StandaloneValue(val) => inputs.push(CliInput::Path(val.into())),

            CliToken::LongFlag("output") | CliToken::ShortFlag("o") => {
                reject_duplicate(&token, &mut output, || lexer.expect_flag_value(&token))?;
            }

            CliToken::LongFlag("entry") | CliToken::ShortFlag("e") => {
                reject_duplicate(&token, &mut entry, || lexer.expect_flag_value(&token))?;
            }

            CliToken::LongFlag("dynamic-linker") => {
                reject_duplicate(&token, &mut dynamic_linker, || lexer.expect_flag_value(&token))?;
            }

            CliToken::LongShortFlag("soname") | CliToken::ShortFlag("h") => {
                reject_duplicate("-soname or -h", &mut shared_object_name, || {
                    lexer.expect_flag_value(&token)
                })?;
            }

            CliToken::LongShortFlag("no-pie") => {
                reject_multiple_modes(&mut mode, Mode::PositionDependent)?;
            }

            CliToken::LongShortFlag("pie") => {
                reject_multiple_modes(&mut mode, Mode::PositionIndependent)?;
            }

            CliToken::LongShortFlag("shared") => {
                reject_multiple_modes(&mut mode, Mode::SharedLibrary)?;
            }

            CliToken::ShortFlag("L") | CliToken::LongFlag("library-path") => {
                let path = lexer.expect_flag_value(&token)?;
                if path.starts_with('=') || path.starts_with("$SYSROOT") {
                    return Err(CliError::UnsupportedSysrootRelativeLibraryPath);
                }
                search_paths.push(PathBuf::from(path));
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
                "relro" => {
                    reject_duplicate("-z relro or -z norelro", &mut read_only_got, || Ok(true))?
                }
                "norelro" => {
                    reject_duplicate("-z relro or -z norelro", &mut read_only_got, || Ok(false))?
                }
                "now" => {
                    reject_duplicate("-z now or -z lazy", &mut read_only_got_plt, || Ok(true))?
                }
                "lazy" => {
                    reject_duplicate("-z now or -z lazy", &mut read_only_got_plt, || Ok(false))?
                }
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
                    ("relocations-analysis", None) => DebugPrint::RelocationsAnalysis,
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

    let mode = mode.unwrap_or(Mode::PositionDependent);

    let options = CliOptions {
        inputs,
        output: output.unwrap_or("a.out").into(),
        gc_sections: gc_sections.unwrap_or(false),
        debug_print,
        search_paths,
        shared_object_name: shared_object_name.map(|s| s.into()),
        executable_stack: executable_stack.unwrap_or(false),
        read_only_got: read_only_got.unwrap_or(false),
        read_only_got_plt: read_only_got_plt.unwrap_or(false),
        mode,

        entry: match mode {
            Mode::PositionDependent | Mode::PositionIndependent => {
                Some(entry.unwrap_or("_start").into())
            }
            Mode::SharedLibrary => None,
        },

        dynamic_linker: match dynamic_linker {
            None => DynamicLinker::PlatformDefault,
            Some(custom) => DynamicLinker::Custom(custom.into()),
        },
    };

    match options.mode {
        Mode::PositionDependent | Mode::SharedLibrary => {
            if options.read_only_got {
                return Err(CliError::RelroOnlyForPie);
            }
            if options.read_only_got_plt {
                return Err(CliError::NowOnlyForPie);
            }
        }
        Mode::PositionIndependent => {}
    }

    match options.mode {
        Mode::PositionDependent | Mode::PositionIndependent => {
            if options.shared_object_name.is_some() {
                return Err(CliError::UnsupportedSharedObjectName);
            }
        }
        Mode::SharedLibrary => {}
    }

    Ok(options)
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
