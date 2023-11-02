use anyhow::{anyhow, bail, Error};
use std::collections::HashMap;
use std::path::Path;
use std::process::{Command, ExitStatus};
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

        for asm in &settings.asm {
            self.compile_asm(root.path(), asm)?;
        }

        match settings.kind {
            TestKind::LinkFail => self.run_link_fail(settings, root),
        }
    }

    fn run_link_fail(self, settings: TestSettings, root: TempDir) -> Result<(), Error> {
        if self.link_and_snapshot(&settings, root.path())?.success() {
            bail!("linking was supposed to fail but passed!");
        }
        Ok(())
    }

    fn link_and_snapshot(&self, settings: &TestSettings, root: &Path) -> Result<ExitStatus, Error> {
        for debug_print in &settings.debug_print {
            let outcome = self.link_and_snapshot_inner(settings, root, Some(&debug_print))?;
            if !outcome.success() {
                anyhow::bail!("debug printing {debug_print} failed, but should always succeed");
            }
        }
        self.link_and_snapshot_inner(settings, root, None)
    }

    fn link_and_snapshot_inner(
        &self,
        settings: &TestSettings,
        root: &Path,
        debug_print: Option<&str>,
    ) -> Result<ExitStatus, Error> {
        let mut command = Command::new(env!("CARGO_BIN_EXE_plink"));
        command.current_dir(root).args(&settings.cmd);
        if let Some(debug_print) = debug_print {
            command.args(["--debug-print", debug_print]);
        }
        let output = command.output()?;

        let mut output_repr = format!("linking exited with {}\n", output.status);
        for (name, content) in [("stdout", &output.stdout), ("stderr", &output.stderr)] {
            if content.is_empty() {
                output_repr.push_str(&format!("\nno {name} present\n"));
            } else {
                output_repr.push_str(&format!(
                    "\n=== {name} ===\n{}\n",
                    String::from_utf8_lossy(content)
                ));
            }
        }

        let mut insta_settings = insta::Settings::clone_current();
        insta_settings.set_prepend_module_to_snapshot(false);
        insta_settings.set_omit_expression(true);
        insta_settings.set_snapshot_path(std::fs::canonicalize(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests")
                .join("linktest")
                .join(self.name),
        )?);
        if let Some(suffix) = debug_print {
            insta_settings.set_snapshot_suffix(suffix);
        }

        insta_settings.bind(|| {
            insta::assert_snapshot!("linker", output_repr);
        });

        Ok(output.status)
    }

    fn compile_asm(&self, root: &Path, asm: &AsmFile) -> Result<(), Error> {
        let source = self
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

        std::fs::write(root.join(&asm.source), source)?;

        eprintln!("compiling {} into {dest_name}...", asm.source);
        run(Command::new("nasm")
            .current_dir(root)
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
