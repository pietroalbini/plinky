mod prerequisites;
mod utils;

use crate::prerequisites::Prerequisites;
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
            self.files.get("test.toml").ok_or_else(|| anyhow!("missing test.toml"))?,
        )?)?;

        for arch in &settings.archs {
            let root = TempDir::new()?;
            TestExecution {
                test: &self,
                settings: &settings,
                root: root.path(),
                suffix: match arch {
                    TestArch::X86 => "-32bit",
                    TestArch::X86_64 => "-64bit",
                },
                arch: *arch,
            }
            .run()?;
        }

        Ok(())
    }
}

struct TestExecution<'a> {
    test: &'a Test,
    settings: &'a TestSettings,
    root: &'a Path,
    suffix: &'a str,
    arch: TestArch,
}

impl TestExecution<'_> {
    fn run(self) -> Result<(), Error> {
        self.settings.prerequisites.build(&self, self.root)?;
        match self.settings.kind {
            TestKind::LinkFail => self.run_link_fail(),
            TestKind::LinkPass => self.run_link_pass(),
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

    fn run_link_pass(self) -> Result<(), Error> {
        if !self.link_and_snapshot()? {
            bail!("linking was supposed to pass but failed!");
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
        let mut command = Command::new(env!("CARGO_BIN_EXE_plinky"));
        command.current_dir(self.root).args(&self.settings.cmd).env("RUST_BACKTRACE", "1");
        for debug_print in &self.settings.debug_print {
            command.args(["--debug-print", debug_print]);
        }

        self.record_snapshot("linker", "linking", &mut command)
    }

    fn run_and_snapshot(&self) -> Result<bool, Error> {
        if !self.link_and_snapshot()? {
            bail!("linking was supposed to pass but failed!");
        }

        let mut command = Command::new(self.root.join("a.out"));
        command.current_dir(self.root);

        self.record_snapshot("run", "running", &mut command)
    }

    fn record_snapshot(
        &self,
        snapshot_name: &str,
        action: &str,
        command: &mut Command,
    ) -> Result<bool, Error> {
        let snapshot_name = format!("{snapshot_name}{}", self.suffix);

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
        insta_settings.set_snapshot_path(std::fs::canonicalize(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("linktest")
                .join("tests")
                .join(self.test.name),
        )?);

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
    #[serde(default = "default_test_archs")]
    archs: Vec<TestArch>,
    #[serde(default)]
    debug_print: Vec<String>,
    #[serde(flatten)]
    prerequisites: Prerequisites,
}

#[derive(serde::Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum TestArch {
    X86,
    X86_64,
}

fn default_test_archs() -> Vec<TestArch> {
    vec![TestArch::X86_64]
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
enum TestKind {
    LinkFail,
    LinkPass,
    RunFail,
    RunPass,
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
