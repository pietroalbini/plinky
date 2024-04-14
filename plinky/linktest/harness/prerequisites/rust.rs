use crate::tests::TestExecution;
use crate::utils::run;
use anyhow::{anyhow, Error};
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RustFile {
    source: String,
    #[serde(default)]
    panic: Panic,
}

impl RustFile {
    pub(super) fn build(&self, execution: &TestExecution, dest_dir: &Path) -> Result<(), Error> {
        let source = execution
            .test
            .files
            .get(&*self.source)
            .ok_or_else(|| anyhow!("missing {}", self.source))?;

        let dest_name = format!(
            "lib{}.a",
            self.source.rsplit_once('.').map(|(name, _ext)| name).unwrap_or(&self.source)
        );

        let source_dir = TempDir::new()?;
        std::fs::write(source_dir.path().join(&self.source), *source)?;

        run(Command::new("rustc")
            .current_dir(source_dir.path())
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

#[derive(serde::Deserialize, Default)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
enum Panic {
    #[default]
    Abort,
}
