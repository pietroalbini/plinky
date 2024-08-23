use anyhow::{bail, Error};
use plinky_test_harness::legacy::prerequisites::{Arch, Prerequisites};
use plinky_test_harness::legacy::{Test, TestGatherer};
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;
use plinky_test_harness::utils::RunAndSnapshot;

struct Linktest;

impl TestGatherer for Linktest {
    const MANIFEST_NAME: &'static str = "test.toml";

    fn tests_for_file(&self, toml_path: &Path) -> Result<Vec<Test>, Error> {
        let path = toml_path.parent().unwrap();
        let test_toml: TestSettings = toml::from_str(&std::fs::read_to_string(&toml_path)?)?;

        let mut tests = Vec::new();
        for arch in &test_toml.archs {
            let name = path.file_name().unwrap().to_string_lossy();
            let arch_name = match arch {
                TestArch::X86 => "x86",
                TestArch::X86_64 => "x86-64",
            };

            let execution = TestExecution::new(path.into(), test_toml.clone(), *arch)?;
            tests.push(Test {
                name: format!("{name} ({arch_name})"),
                exec: Box::new(move || execution.exec()),
                ignore: test_toml.ignore.clone(),
            });
        }
        Ok(tests)
    }
}

struct TestExecution {
    root: PathBuf,
    settings: TestSettings,
    arch: TestArch,
    dest_dir: PathBuf,
}

impl TestExecution {
    fn new(root: PathBuf, settings: TestSettings, arch: TestArch) -> Result<Self, Error> {
        Ok(Self {
            root,
            settings,
            arch,
            // Don't rely on TempDir's automatic deletion. Instead, we delete the directory only if
            // the tests execute successfully. This allows inspecting the intermediate files.
            dest_dir: TempDir::new()?.into_path(),
        })
    }

    fn exec(self) -> Result<(), Error> {
        let dest_dir = self.dest_dir.clone();
        println!("building prerequisites in {}", dest_dir.display());

        self.settings.prerequisites.build(
            match self.arch {
                TestArch::X86 => Arch::X86,
                TestArch::X86_64 => Arch::X86_64,
            },
            &self.root,
            &dest_dir,
        )?;

        let (res, err) = match self.settings.kind {
            TestKind::LinkFail => (!self.link()?, "linking was supposed to fail but passed!"),
            TestKind::LinkPass => (self.link()?, "linking was supposed to pass but failed!"),
            TestKind::RunFail => (!self.run()?, "running was supposed to fail but passed!"),
            TestKind::RunPass => (self.run()?, "running was supposed to pass but failed!"),
        };
        if res {
            let _ = std::fs::remove_dir_all(&dest_dir);
            Ok(())
        } else {
            bail!("{err}");
        }
    }

    fn link(&self) -> Result<bool, Error> {
        let mut command = Command::new(env!("CARGO_BIN_EXE_ld.plinky"));
        command.current_dir(&self.dest_dir).args(&self.settings.cmd).env("RUST_BACKTRACE", "1");
        for debug_print in &self.settings.debug_print {
            command.args(["--debug-print", debug_print]);
        }

        self.record_snapshot("linker", "linking", &mut command)
    }

    fn run(&self) -> Result<bool, Error> {
        if !self.link()? {
            bail!("linking was supposed to pass but failed!");
        }

        let mut command = Command::new(self.dest_dir.join("a.out"));
        command.current_dir(&self.dest_dir);

        self.record_snapshot("run", "running", &mut command)
    }

    fn record_snapshot(
        &self,
        name: &str,
        action: &str,
        command: &mut Command,
    ) -> Result<bool, Error> {
        let mut runner = RunAndSnapshot::new(&format!("{name}{}", self.suffix()), &self.root);
        let outcome = runner.run(action, command)?;
        runner.persist();
        Ok(outcome)
    }

    fn suffix(&self) -> &'static str {
        match &self.arch {
            TestArch::X86 => "-32bit",
            TestArch::X86_64 => "-64bit",
        }
    }
}

#[derive(serde::Deserialize, Clone)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
struct TestSettings {
    #[serde(default)]
    ignore: Option<String>,
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

#[derive(serde::Deserialize, Clone, Copy)]
#[serde(rename_all = "kebab-case")]
enum TestKind {
    LinkFail,
    LinkPass,
    RunFail,
    RunPass,
}

fn main() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("linktest");
    plinky_test_harness::legacy::main(&path, Linktest);
}
