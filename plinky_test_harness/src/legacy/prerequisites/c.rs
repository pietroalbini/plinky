use crate::legacy::prerequisites::Arch;
use crate::utils::run;
use anyhow::Error;
use std::path::Path;
use std::process::Command;

#[derive(serde::Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub(super) struct CFile {
    source: String,
    output: Option<String>,
    libc: Libc,
    relocation: Relocation,
}

impl CFile {
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
        run(Command::new("cc")
            .current_dir(source_dir)
            .arg("-c")
            // Disable control-flow protection, as it's not implemented in the linker right now.
            .arg("-fcf-protection=none")
            .arg("-o")
            .arg(dest_dir.join(dest_name))
            .arg(match arch {
                Arch::X86 => "-m32",
                Arch::X86_64 => "-m64",
            })
            .args(match self.libc {
                Libc::Freestanding => &["-nostdlib"],
            })
            .args(match self.relocation {
                Relocation::Static => &["-fno-pic"] as &[&str],
                Relocation::PicOnlyGot => &["-fPIC", "-fno-plt"],
                Relocation::Pic => &["-fPIC"],
            })
            .arg(&self.source))?;

        Ok(())
    }
}

#[derive(serde::Deserialize, Debug, Clone)]
#[serde(rename_all = "kebab-case")]
enum Libc {
    Freestanding,
}

#[derive(serde::Deserialize, Debug, Clone)]
#[serde(rename_all = "kebab-case")]
enum Relocation {
    Static,
    PicOnlyGot,
    Pic,
}
