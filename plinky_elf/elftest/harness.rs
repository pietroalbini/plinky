use anyhow::{bail, Error};
use plinky_elf::ids::serial::SerialIds;
use plinky_elf::ElfObject;
use plinky_test_harness::prerequisites::{Arch, Prerequisites};
use plinky_test_harness::utils::record_snapshot;
use plinky_test_harness::{Test, TestGatherer};
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

struct Elftest;

impl TestGatherer for Elftest {
    const MANIFEST_NAME: &'static str = "test.toml";

    fn tests_for_file(&self, toml_path: &Path) -> Result<Vec<Test>, anyhow::Error> {
        let path = toml_path.parent().unwrap();
        let test_toml: TestToml = toml::from_str(&std::fs::read_to_string(toml_path)?)?;

        let mut tests = Vec::new();
        for arch in &test_toml.archs {
            let execution = TestExecution {
                source_dir: path.into(),
                // Don't rely on TempDir's automatic deletion. Instead, we delete the directory only if
                // the tests execute successfully. This allows inspecting the intermediate files.
                dest_dir: TempDir::new()?.into_path(),
                toml: test_toml.clone(),
                arch: *arch,
            };
            tests.push(Test {
                name: format!(
                    "{} ({})",
                    path.file_name().unwrap().to_str().unwrap(),
                    match arch {
                        Arch::X86 => "x86",
                        Arch::X86_64 => "x86-64",
                    }
                ),
                exec: Box::new(move || execution.run()),
                ignore: None,
            });
        }

        Ok(tests)
    }
}

struct TestExecution {
    source_dir: PathBuf,
    dest_dir: PathBuf,
    toml: TestToml,
    arch: Arch,
}

impl TestExecution {
    fn run(&self) -> Result<(), Error> {
        println!("building prerequisites in {}", self.dest_dir.display());
        self.toml.prerequisites.build(self.arch, &self.source_dir, &self.dest_dir)?;

        insta::allow_duplicates! {
            self.read(&self.toml.read)?;
            if self.toml.roundtrip {
                let roundtrip = self.roundtrip(&self.toml.read)?;
                self.read(&roundtrip)?;
            }

            Ok::<(), Error>(())
        }?;

        let _ = std::fs::remove_dir_all(&self.dest_dir);
        Ok(())
    }

    fn roundtrip(&self, file: &Path) -> Result<PathBuf, Error> {
        println!("writing the file back for the roundtrip...");

        let dest = self.dest_dir.join("roundtrip").join(file);
        std::fs::create_dir_all(dest.parent().unwrap())?;

        let mut ids = SerialIds::new();
        ElfObject::load(&mut BufReader::new(File::open(self.dest_dir.join(file))?), &mut ids)?
            .write(&mut BufWriter::new(File::create_new(&dest)?))?;

        Ok(dest)
    }

    fn read(&self, file: &Path) -> Result<(), Error> {
        println!("reading {}...", file.display());

        let mut command = Command::new(env!("CARGO_BIN_EXE_read-elf"));
        command.current_dir(&self.dest_dir).arg(file);
        if let Some(filter) = &self.toml.filter {
            command.arg(filter);
        }

        if !self.record_snapshot("read", "reading", &mut command)? {
            bail!("failed to read the ELF file");
        }
        Ok(())
    }

    fn record_snapshot(
        &self,
        name: &str,
        action: &str,
        command: &mut Command,
    ) -> Result<bool, Error> {
        let suffix = match &self.arch {
            Arch::X86 => "-32bit",
            Arch::X86_64 => "-64bit",
        };
        record_snapshot(&format!("{name}{suffix}"), &self.source_dir, action, command)
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct TestToml {
    read: PathBuf,
    archs: Vec<Arch>,
    #[serde(default = "default_true")]
    roundtrip: bool,
    #[serde(default)]
    filter: Option<String>,
    #[serde(flatten)]
    prerequisites: Prerequisites,
}

fn default_true() -> bool {
    true
}

fn main() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("elftest");
    plinky_test_harness::main(&path, Elftest);
}
