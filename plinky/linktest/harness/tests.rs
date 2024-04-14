use crate::prerequisites::Prerequisites;
use anyhow::{bail, Context as _, Error};
use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;

pub(crate) struct TestExecution {
    root: PathBuf,
    settings: TestSettings,
    pub(crate) arch: TestArch,
    dest_dir: TempDir,
}

impl TestExecution {
    pub(crate) fn new(
        root: PathBuf,
        settings: TestSettings,
        arch: TestArch,
    ) -> Result<Self, Error> {
        Ok(Self { root, settings, arch, dest_dir: TempDir::new()? })
    }

    pub(crate) fn run(self) -> Result<(), Error> {
        self.settings.prerequisites.build(&self, self.dest_dir.path())?;
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
        let mut command = Command::new(env!("CARGO_BIN_EXE_ld.plinky"));
        command
            .current_dir(self.dest_dir.path())
            .args(&self.settings.cmd)
            .env("RUST_BACKTRACE", "1");
        for debug_print in &self.settings.debug_print {
            command.args(["--debug-print", debug_print]);
        }

        self.record_snapshot("linker", "linking", &mut command)
    }

    fn run_and_snapshot(&self) -> Result<bool, Error> {
        if !self.link_and_snapshot()? {
            bail!("linking was supposed to pass but failed!");
        }

        let mut command = Command::new(self.dest_dir.path().join("a.out"));
        command.current_dir(self.dest_dir.path());

        self.record_snapshot("run", "running", &mut command)
    }

    fn record_snapshot(
        &self,
        snapshot_name: &str,
        action: &str,
        command: &mut Command,
    ) -> Result<bool, Error> {
        let snapshot_name = format!("{snapshot_name}{}", self.suffix());

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
        insta_settings.set_snapshot_path(self.root.canonicalize()?);

        insta_settings.bind(|| {
            insta::assert_snapshot!(snapshot_name, output_repr);
        });
        Ok(success)
    }

    fn suffix(&self) -> &'static str {
        match &self.arch {
            TestArch::X86 => "-32bit",
            TestArch::X86_64 => "-64bit",
        }
    }

    pub(crate) fn file(&self, name: &str) -> Result<Vec<u8>, Error> {
        std::fs::read(self.root.join(name)).with_context(|| format!("failed to read file {name}"))
    }
}

#[derive(serde::Deserialize, Clone)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub(crate) struct TestSettings {
    cmd: Vec<String>,
    kind: TestKind,
    #[serde(default = "default_test_archs")]
    pub(crate) archs: Vec<TestArch>,
    #[serde(default)]
    debug_print: Vec<String>,
    #[serde(flatten)]
    prerequisites: Prerequisites,
}

#[derive(serde::Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TestArch {
    X86,
    X86_64,
}

fn default_test_archs() -> Vec<TestArch> {
    vec![TestArch::X86_64]
}

#[derive(serde::Deserialize, Clone, Copy)]
#[serde(rename_all = "kebab-case")]
enum TestKind {
    LinkFail,
    LinkPass,
    RunFail,
    RunPass,
}
