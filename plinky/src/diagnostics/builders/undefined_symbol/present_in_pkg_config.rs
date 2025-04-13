use crate::cli::CliError;
use crate::diagnostics::builders::UndefinedSymbolDiagnostic;
use crate::utils::file_type::{FileType, FileTypeError};
use crate::utils::resolve_cli_input::{resolve_cli_input, ResolveCliInputError, ResolvedInput};
use plinky_ar::{ArReadError, ArReader};
use plinky_diagnostics::widgets::{Table, Text, Widget};
use plinky_diagnostics::GatheredContext;
use plinky_elf::{ElfReader, ElfType, LoadError, ReadDynamicError};
use plinky_macros::{Display, Error};
use plinky_pkg_config::{discover, ParseError, PkgConfig, PkgConfigEnv};
use plinky_utils::posix_shell_quote;
use std::error::Error;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

pub(super) fn generate(
    diagnostic: &UndefinedSymbolDiagnostic,
    _ctx: &GatheredContext<'_>,
) -> Vec<Box<dyn Widget>> {
    let packages = match PkgConfigEnv::from_env().and_then(|env| discover(&env)) {
        Ok(packages) => packages,
        Err(err) => {
            return vec![text(&format!(
                "note: failed to discover packages possibly containing this symbol: {}",
                one_line_error(&err),
            ))];
        }
    };

    let mut errors = Table::new();
    errors.add_head(["Package name", "pkg-config file", "Error message"]);
    let mut found = Table::new();
    found.add_head(["Package name", "Linker flags"]);

    let search_for = diagnostic.name.resolve();
    for (name, path) in packages {
        match process_pkg_config(&path, &search_for) {
            Ok(Some(flags)) => found.add_body([name, flags]),
            Ok(None) => {}
            Err(err) => errors.add_body([name, path.display().to_string(), one_line_error(&err)]),
        }
    }

    let mut result = Vec::new();
    if !found.is_body_empty() {
        result.push(text("help: the following libraries provide the symbol:"));
        result.push(Box::new(found));
    }
    if !errors.is_body_empty() {
        result.push(text("note: could not check if these libraries contained the symbol:"));
        result.push(Box::new(errors));
    }
    result
}

fn process_pkg_config(
    path: &Path,
    search_for: &str,
) -> Result<Option<String>, ProcessPkgConfigError> {
    let file = std::fs::read_to_string(path).map_err(ProcessPkgConfigError::ReadPkgConfig)?;
    let Some(libs) = PkgConfig::parse(&file)?.libs else { return Ok(None) };
    let options = crate::cli::parse(libs.iter())?;

    let mut found = false;
    for lib in options.inputs {
        let ResolvedInput { path, .. } = resolve_cli_input(&options.search_paths, &lib)?;
        let mut reader = BufReader::new(
            File::open(&path).map_err(|e| ProcessPkgConfigError::ReadLibrary(path.clone(), e))?,
        );

        match FileType::from_magic_number(&path, &mut reader)? {
            FileType::Elf => {
                let mut reader = ElfReader::new(&mut reader)
                    .map_err(|e| ProcessPkgConfigError::ReadElf(path.clone(), e))?;
                match reader.type_() {
                    ElfType::SharedObject => {
                        let has_symbol = reader
                            .dynamic()
                            .and_then(|mut d| d.has_symbol(search_for))
                            .map_err(|e| ProcessPkgConfigError::ReadElfDynamic(path.clone(), e))?;
                        if has_symbol {
                            found = true;
                            break;
                        }
                    }
                    ElfType::Relocatable | ElfType::Executable | ElfType::Core => continue,
                }
            }
            FileType::Ar => {
                let archive = ArReader::new(reader)
                    .map_err(|e| ProcessPkgConfigError::ReadArchive(path.clone(), e))?;
                if let Some(symbol_table) = archive.symbol_table() {
                    if symbol_table.symbols.contains_key(search_for) {
                        found = true;
                        break;
                    }
                }
            }
        }
    }

    if found {
        Ok(Some(libs.iter().map(|s| posix_shell_quote(s)).collect::<Vec<_>>().join(" ")))
    } else {
        Ok(None)
    }
}

fn text(input: &str) -> Box<dyn Widget> {
    Box::new(Text::new(input))
}

fn one_line_error(mut err: &dyn Error) -> String {
    let mut output = String::new();
    loop {
        if !output.is_empty() {
            output.push_str(": ");
        }
        output.push_str(&format!("{err}"));

        if let Some(source) = err.source() {
            err = source;
        } else {
            break;
        }
    }
    output
}

#[derive(Debug, Error, Display)]
enum ProcessPkgConfigError {
    #[display("failed to read pkg-config")]
    ReadPkgConfig(#[source] std::io::Error),
    #[display("failed to read library {f0:?}")]
    ReadLibrary(PathBuf, #[source] std::io::Error),
    #[display("failed to read object {f0:?}")]
    ReadElf(PathBuf, LoadError),
    #[display("failed to read object {f0:?}")]
    ReadElfDynamic(PathBuf, ReadDynamicError),
    #[display("failed to read archive {f0:?}")]
    ReadArchive(PathBuf, #[source] ArReadError),
    #[transparent]
    Parse(ParseError),
    #[display("invalid Libs field")]
    InvalidLibs(#[from] CliError),
    #[transparent]
    ResolveCliInput(ResolveCliInputError),
    #[transparent]
    FileType(FileTypeError),
}
