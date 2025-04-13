use crate::diagnostics::builders::UndefinedSymbolDiagnostic;
use plinky_diagnostics::widgets::{Table, Text, Widget};
use plinky_diagnostics::GatheredContext;
use plinky_macros::{Display, Error};
use plinky_pkg_config::{discover, ParseError, PkgConfig, PkgConfigEnv};
use std::error::Error;
use std::path::Path;

pub(super) fn generate(
    _diagnostic: &UndefinedSymbolDiagnostic,
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

    for (name, path) in packages {
        match process_pkg_config(&path) {
            Ok(flags) => found.add_body([name, flags]),
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

fn process_pkg_config(path: &Path) -> Result<String, ProcessPkgConfigError> {
    let file = std::fs::read_to_string(path).map_err(ProcessPkgConfigError::ReadFile)?;
    let _pkg = PkgConfig::parse(&file)?;
    Ok("TODO".into())
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
    ReadFile(#[source] std::io::Error),
    #[transparent]
    Parse(ParseError),
}
