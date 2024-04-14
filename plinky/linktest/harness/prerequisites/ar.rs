use crate::prerequisites::Prerequisites;
use crate::tests::TestExecution;
use crate::utils::run;
use anyhow::Error;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub(super) struct ArArchive {
    name: String,
    #[serde(default)]
    symbol_table: Option<bool>,
    #[serde(flatten)]
    content: Prerequisites,
}

impl ArArchive {
    pub(super) fn build(&self, execution: &TestExecution, dest_dir: &Path) -> Result<(), Error> {
        let inputs_dir = TempDir::new()?;
        self.content.build(execution, inputs_dir.path())?;

        let mut to_archive = Vec::new();
        for entry in std::fs::read_dir(inputs_dir.path())? {
            let path = entry?.path();
            if path.is_file() {
                to_archive.push(path.file_name().unwrap().to_os_string());
            }
        }
        to_archive.sort();

        let mut flags = "rc".to_string();
        match self.symbol_table {
            None | Some(true) => flags.push('s'),
            Some(false) => flags.push('S'),
        }

        println!("archiving {to_archive:?} into {}...", self.name);
        run(Command::new("ar")
            .current_dir(inputs_dir.path())
            .arg(flags)
            .arg(dest_dir.join(&self.name))
            .args(&to_archive))?;
        Ok(())
    }
}
