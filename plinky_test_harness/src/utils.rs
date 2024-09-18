use anyhow::{bail, Error};
use std::path::{Path, PathBuf};
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

pub struct RunAndSnapshot {
    name: String,
    path: PathBuf,
    output: String,
}

impl RunAndSnapshot {
    pub fn new(name: &str, path: &Path) -> Self {
        Self { name: name.into(), path: path.into(), output: String::new() }
    }

    pub fn run(&mut self, action: &str, command: &mut Command) -> Result<bool, Error> {
        self.separator();
        match command.output() {
            Ok(output) => {
                self.output.push_str(&format!("{action} exited with {}\n", output.status));
                for (name, content) in [("stdout", &output.stdout), ("stderr", &output.stderr)] {
                    if content.trim_ascii().is_empty() {
                        self.output.push_str(&format!("\nno {name} present\n"));
                    } else {
                        let mut content = String::from_utf8_lossy(content).to_string();
                        content = content.replace(env!("CARGO_MANIFEST_DIR"), "${project}");

                        self.output
                            .push_str(&format!("\n=== {name} ===\n{}\n", content.trim_ascii_end()));
                    }
                }
                Ok(output.status.success())
            }
            Err(err) => {
                self.output.push_str(&format!("{action} failed to execute with error: {err}"));
                Ok(false)
            }
        }
    }

    pub fn note(&mut self, note: &str) {
        self.separator();
        self.output.push_str(note);
        if !note.ends_with('\n') {
            self.output.push('\n');
        }
    }

    fn separator(&mut self) {
        if !self.output.is_empty() {
            self.output.push_str("\n==============\n\n");
        }
    }

    pub fn persist(self) {
        let mut insta_settings = insta::Settings::clone_current();
        insta_settings.set_prepend_module_to_snapshot(false);
        insta_settings.set_omit_expression(true);
        insta_settings.set_snapshot_path(self.path.canonicalize().unwrap());

        insta_settings.bind(|| {
            insta::assert_snapshot!(self.name, self.output.trim_ascii_end());
        });
    }
}

pub(crate) fn file_name(path: impl AsRef<Path>) -> String {
    path.as_ref().file_name().unwrap().to_str().unwrap().to_string()
}

#[track_caller]
pub(crate) fn err_str<T>(result: Result<T, Error>) -> Result<T, String> {
    match result {
        Ok(ok) => Ok(ok),
        Err(err) => {
            let mut repr = format!("error: {err}\n");
            let mut source = err.source();
            while let Some(err) = source {
                repr.push_str(&format!("  cause: {err}\n"));
                source = err.source();
            }
            panic!("{repr}")
        }
    }
}
