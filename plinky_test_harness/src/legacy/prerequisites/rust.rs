use crate::legacy::prerequisites::Arch;
use crate::utils::run;
use anyhow::Error;
use std::path::Path;
use std::process::Command;

#[derive(serde::Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub(super) struct RustFile {
    source: String,
    #[serde(default)]
    panic: Panic,
}

impl RustFile {
    pub(super) fn build(
        &self,
        _arch: Arch,
        source_dir: &Path,
        dest_dir: &Path,
    ) -> Result<(), Error> {
        let dest_name = format!(
            "lib{}.a",
            self.source.rsplit_once('.').map(|(name, _ext)| name).unwrap_or(&self.source)
        );

        run(Command::new("rustc")
            .current_dir(source_dir)
            .arg(&self.source)
            .arg("-o")
            .arg(dest_dir.join(dest_name))
            .arg("--crate-type=staticlib")
            .arg(match self.panic {
                Panic::Abort => "-Cpanic=abort",
            }))?;

        Ok(())
    }
}

#[derive(serde::Deserialize, Debug, Default, Clone, Copy)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
enum Panic {
    #[default]
    Abort,
}
