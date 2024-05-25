mod ar;
mod asm;
mod c;
mod ld;
mod rust;

use crate::prerequisites::ar::ArArchive;
use crate::prerequisites::asm::AsmFile;
use crate::prerequisites::c::CFile;
use crate::prerequisites::ld::LdInvocation;
use crate::prerequisites::rust::RustFile;
use anyhow::Error;
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Deserialize)]
pub enum Arch {
    #[serde(rename = "x86")]
    X86,
    #[serde(rename = "x86_64")]
    X86_64,
}

#[derive(serde::Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct Prerequisites {
    #[serde(default)]
    asm: Vec<AsmFile>,
    #[serde(default)]
    c: Vec<CFile>,
    #[serde(default)]
    ar: Vec<ArArchive>,
    #[serde(default)]
    ld: Vec<LdInvocation>,
    #[serde(default)]
    rust: Vec<RustFile>,
    #[serde(default)]
    arch: BTreeMap<Arch, Prerequisites>,
}

impl Prerequisites {
    pub fn build(&self, arch: Arch, source_dir: &Path, dest_dir: &Path) -> Result<(), Error> {
        for asm in &self.asm {
            asm.build(arch, source_dir, dest_dir)?;
        }
        for c in &self.c {
            c.build(arch, source_dir, dest_dir)?;
        }
        for ar in &self.ar {
            ar.build(arch, source_dir, dest_dir)?;
        }
        for rust in &self.rust {
            rust.build(arch, source_dir, dest_dir)?;
        }
        for ld in &self.ld {
            ld.build(arch, source_dir, dest_dir)?;
        }
        if let Some(arch_specific) = self.arch.get(&arch) {
            arch_specific.build(arch, source_dir, dest_dir)?;
        }
        Ok(())
    }
}
