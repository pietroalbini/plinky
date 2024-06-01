use crate::prerequisites::{Arch, Prerequisites};
use crate::utils::run;
use anyhow::Error;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

#[derive(serde::Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub(super) struct LdInvocation {
    dest: String,
    #[serde(default)]
    shared_library: bool,
    #[serde(flatten)]
    content: Prerequisites,
}

impl LdInvocation {
    pub(super) fn build(
        &self,
        arch: Arch,
        source_dir: &Path,
        dest_dir: &Path,
    ) -> Result<(), Error> {
        let inputs_dir = TempDir::with_prefix_in("prereq-", dest_dir)?.into_path();
        self.content.build(arch, source_dir, &inputs_dir)?;

        let mut to_link = Vec::new();
        for entry in std::fs::read_dir(&inputs_dir)? {
            let path = entry?.path();
            if path.is_file() {
                to_link.push(path.file_name().unwrap().to_os_string());
            }
        }
        to_link.sort();

        println!("linking {to_link:?} into {}...", self.dest);
        run(Command::new("ld")
            .current_dir(&inputs_dir)
            .arg("-o")
            .arg(dest_dir.join(&self.dest))
            .args(to_link)
            .args(if self.shared_library { &["-shared"] as &[_] } else { &[] })
            .args(match arch {
                Arch::X86 => ["-m", "elf_i386"],
                Arch::X86_64 => ["-m", "elf_x86_64"],
            }))?;
        Ok(())
    }
}
