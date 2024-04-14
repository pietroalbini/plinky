use anyhow::{bail, Error};
use std::process::Command;

pub(crate) fn run(command: &mut Command) -> Result<(), Error> {
    let cmd_repr = format!("{command:?}");
    let output = command.output()?;
    if !output.status.success() {
        eprintln!("Failed to execute {cmd_repr}");
        eprintln!();
        eprintln!("=== stdout ===");
        eprintln!("{}", String::from_utf8_lossy(&output.stdout));
        eprintln!();
        eprintln!("=== stderr ===");
        eprintln!("{}", String::from_utf8_lossy(&output.stderr));
        eprintln!();
        bail!("command failed with exit {}", output.status);
    }
    Ok(())
}

pub(crate) fn err_str<T>(result: Result<T, Error>) -> Result<T, String> {
    result.map_err(|err| {
        let mut repr = format!("error: {err}\n");
        let mut source = err.source();
        while let Some(err) = source {
            repr.push_str(&format!("  cause: {err}\n"));
            source = err.source();
        }
        repr
    })
}
