use crate::tests::{TestArch, TestExecution};
use crate::utils::run;
use anyhow::{anyhow, Error};
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct AsmFile {
    source: String,
    format: Option<AsmFormat>,
    output: Option<String>,
}

impl AsmFile {
    pub(super) fn build(&self, execution: &TestExecution, dest_dir: &Path) -> Result<(), Error> {
        let source = execution
            .test
            .files
            .get(&*self.source)
            .ok_or_else(|| anyhow!("missing {}", self.source))?;

        let dest_name = match &self.output {
            Some(output) => output.clone(),
            None => format!(
                "{}.o",
                self.source.rsplit_once('.').map(|(name, _ext)| name).unwrap_or(&self.source)
            ),
        };

        let source_dir = TempDir::new()?;
        std::fs::write(source_dir.path().join(&self.source), *source)?;

        eprintln!("compiling {} into {dest_name}...", self.source);
        run(Command::new("nasm")
            .current_dir(source_dir.path())
            .arg("-f")
            .arg(match (&self.format, execution.arch) {
                (Some(AsmFormat::Elf32), _) | (None, TestArch::X86) => "elf32",
                (Some(AsmFormat::Elf64), _) | (None, TestArch::X86_64) => "elf64",
            })
            .arg("-o")
            .arg(dest_dir.join(dest_name))
            .arg(&self.source))?;

        Ok(())
    }
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
enum AsmFormat {
    Elf32,
    Elf64,
}
