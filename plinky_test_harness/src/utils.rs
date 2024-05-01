use anyhow::{bail, Error};
use std::path::Path;
use std::process::Command;

pub fn run(command: &mut Command) -> Result<(), Error> {
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

pub fn record_snapshot(
    snapshot_name: &str,
    snapshot_path: &Path,
    action: &str,
    command: &mut Command,
) -> Result<bool, Error> {
    let (output_repr, success) = match command.output() {
        Ok(output) => {
            let mut output_repr = format!("{action} exited with {}\n", output.status);
            for (name, content) in [("stdout", &output.stdout), ("stderr", &output.stderr)] {
                if content.is_empty() {
                    output_repr.push_str(&format!("\nno {name} present\n"));
                } else {
                    let content = String::from_utf8_lossy(content);
                    let content = content.replace(env!("CARGO_MANIFEST_DIR"), "${project}");

                    output_repr.push_str(&format!("\n=== {name} ===\n{}\n", content,));
                }
            }
            (output_repr, output.status.success())
        }
        Err(err) => (format!("{action} failed to execute with error: {err}"), false),
    };

    let mut insta_settings = insta::Settings::clone_current();
    insta_settings.set_prepend_module_to_snapshot(false);
    insta_settings.set_omit_expression(true);
    insta_settings.set_snapshot_path(snapshot_path.canonicalize()?);

    insta_settings.bind(|| {
        insta::assert_snapshot!(snapshot_name, output_repr);
    });
    Ok(success)
}

#[track_caller]
pub fn err_str<T>(result: Result<T, Error>) -> Result<T, String> {
    result.map_err(|err| {
        let mut repr = format!("error: {err}\n");
        let mut source = err.source();
        while let Some(err) = source {
            repr.push_str(&format!("  cause: {err}\n"));
            source = err.source();
        }
        panic!("{repr}")
    })
}
