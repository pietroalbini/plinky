use crate::legacy::prerequisites::Arch;
use crate::utils::run;
use anyhow::Error;
use std::path::Path;
use std::process::Command;

#[derive(serde::Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub(super) struct AsmFile {
    source: String,
    format: Option<AsmFormat>,
    output: Option<String>,
}

impl AsmFile {
    pub(super) fn build(
        &self,
        arch: Arch,
        source_dir: &Path,
        dest_dir: &Path,
    ) -> Result<(), Error> {
        let dest_name = match &self.output {
            Some(output) => output.clone(),
            None => format!(
                "{}.o",
                self.source.rsplit_once('.').map(|(name, _ext)| name).unwrap_or(&self.source)
            ),
        };

        eprintln!("compiling {} into {dest_name}...", self.source);
        run(Command::new("as")
            .current_dir(source_dir)
            .arg(match (&self.format, arch) {
                (Some(AsmFormat::Elf32), _) | (None, Arch::X86) => "--32",
                (Some(AsmFormat::Elf64), _) | (None, Arch::X86_64) => "--64",
            })
            .arg("-o")
            .arg(dest_dir.join(dest_name))
            .arg(&self.source))?;

        Ok(())
    }
}

#[derive(serde::Deserialize, Debug, Clone, Copy)]
#[serde(rename_all = "kebab-case")]
enum AsmFormat {
    Elf32,
    Elf64,
}
