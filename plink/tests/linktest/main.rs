use anyhow::{anyhow, bail, Error};
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

struct Test {
    name: &'static str,
    files: HashMap<&'static str, &'static [u8]>,
}

impl Test {
    fn run(self) -> Result<(), Error> {
        let settings: TestSettings = toml::from_str(std::str::from_utf8(
            self.files
                .get("test.toml")
                .ok_or_else(|| anyhow!("missing test.toml"))?,
        )?)?;

        let root = TempDir::new()?;

        TestExecution {
            test: &self,
            settings: &settings,
            root: root.path(),
            suffix: "-64bit",
        }

        .run()?;

        Ok(())
    }
}

struct TestExecution<'a> {
    test: &'a Test,
    settings: &'a TestSettings,
    root: &'a Path,
    suffix: &'a str,
}

impl TestExecution<'_> {
    fn run(self) -> Result<(), Error> {
        for asm in &self.settings.asm {
            self.compile_asm(asm)?;
        }

        match self.settings.kind {
            TestKind::LinkFail => self.run_link_fail(),
            TestKind::RunFail => self.run_run_fail(),
            TestKind::RunPass => self.run_run_pass(),
        }
    }

    fn run_link_fail(self) -> Result<(), Error> {
        if self.link_and_snapshot()? {
            bail!("linking was supposed to fail but passed!");
        }
        Ok(())
    }

    fn run_run_fail(self) -> Result<(), Error> {
        if self.run_and_snapshot()? {
            bail!("running was supposed to fail but passed!");
        }
        Ok(())
    }

    fn run_run_pass(self) -> Result<(), Error> {
        if !self.run_and_snapshot()? {
            bail!("running was supposed to pass but failed!");
        }
        Ok(())
    }

    fn link_and_snapshot(&self) -> Result<bool, Error> {
        for debug_print in &self.settings.debug_print {
            let outcome = self.link_and_snapshot_inner(Some(&debug_print))?;
            if !outcome {
                anyhow::bail!("debug printing {debug_print} failed, but should always succeed");
            }
        }
        self.link_and_snapshot_inner(None)
    }

    fn run_and_snapshot(&self) -> Result<bool, Error> {
        if !self.link_and_snapshot()? {
            bail!("linking was supposed to pass but failed!");
        }

        let mut command = Command::new(self.root.join("a.out"));
        command.current_dir(self.root);

        self.record_snapshot("run", "running", &mut command, None)
    }

    fn link_and_snapshot_inner(&self, debug_print: Option<&str>) -> Result<bool, Error> {
        let mut command = Command::new(env!("CARGO_BIN_EXE_plink"));
        command
            .current_dir(self.root)
            .args(&self.settings.cmd)
            .env("RUST_BACKTRACE", "1");
        if let Some(debug_print) = debug_print {
            command.args(["--debug-print", debug_print]);
        }

        self.record_snapshot("linker", "linking", &mut command, debug_print)
    }

    fn compile_asm(&self, asm: &AsmFile) -> Result<(), Error> {
        let source = self
            .test
            .files
            .get(&*asm.source)
            .ok_or_else(|| anyhow!("missing {}", asm.source))?;

        let dest_name = match &asm.output {
            Some(output) => output.clone(),
            None => format!(
                "{}.o",
                asm.source
                    .rsplit_once('.')
                    .map(|(name, _ext)| name)
                    .unwrap_or(&asm.source)
            ),
        };

        std::fs::write(self.root.join(&asm.source), source)?;

        eprintln!("compiling {} into {dest_name}...", asm.source);
        run(Command::new("nasm")
            .current_dir(self.root)
            .arg("-f")
            .arg(match asm.format {
                AsmFormat::Elf32 => "elf32",
                AsmFormat::Elf64 => "elf64",
            })
            .arg("-o")
            .arg(&dest_name)
            .arg(&asm.source))?;

        Ok(())
    }

    fn record_snapshot(
        &self,
        snapshot_name: &str,
        action: &str,
        command: &mut Command,
        suffix: Option<&str>,
    ) -> Result<bool, Error> {
        let snapshot_name = format!("{snapshot_name}{}", self.suffix);

        let (output_repr, success) = match command.output() {
            Ok(output) => {
                let mut output_repr = format!("{action} exited with {}\n", output.status);
                for (name, content) in [("stdout", &output.stdout), ("stderr", &output.stderr)] {
                    if content.is_empty() {
                        output_repr.push_str(&format!("\nno {name} present\n"));
                    } else {
                        let content = String::from_utf8_lossy(&content);
                        let content = content.replace(env!("CARGO_MANIFEST_DIR"), "${project}");

                        output_repr.push_str(&format!("\n=== {name} ===\n{}\n", content,));
                    }
                }
                (output_repr, output.status.success())
            }
            Err(err) => (
                format!("{action} failed to execute with error: {err}"),
                false,
            ),
        };

        let mut insta_settings = insta::Settings::clone_current();
        insta_settings.set_prepend_module_to_snapshot(false);
        insta_settings.set_omit_expression(true);
        insta_settings.set_snapshot_path(std::fs::canonicalize(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests")
                .join("linktest")
                .join(self.test.name),
        )?);
        if let Some(suffix) = suffix {
            insta_settings.set_snapshot_suffix(suffix);
        }

        insta_settings.bind(|| {
            insta::assert_snapshot!(snapshot_name, output_repr);
        });
        Ok(success)
    }
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
struct TestSettings {
    cmd: Vec<String>,
    kind: TestKind,
    #[serde(default)]
    debug_print: Vec<String>,
    #[serde(default)]
    asm: Vec<AsmFile>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
enum TestKind {
    LinkFail,
    RunFail,
    RunPass,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct AsmFile {
    source: String,
    #[serde(default)]
    format: AsmFormat,
    output: Option<String>,
}

#[derive(serde::Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
enum AsmFormat {
    Elf32,
    #[default]
    Elf64,
}

fn run(command: &mut Command) -> Result<(), Error> {
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

macro_rules! linktest {
    ($name:ident, files[$($file:expr),*]) => {
        #[test]
        fn $name() {
            let mut files = HashMap::new();
            $(files.insert($file.rsplit_once('/').unwrap().1, include_bytes!($file) as &[u8]);)*
            Test {
                name: stringify!($name),
                files,
            }.run().unwrap();
        }
    }
}

include!(concat!(env!("OUT_DIR"), "/linktest_definition.rs"));
