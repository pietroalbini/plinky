mod ar;
mod asm;
mod c;

use crate::prerequisites::ar::ArArchive;
use crate::prerequisites::asm::AsmFile;
use crate::prerequisites::c::CFile;
use crate::{TestArch, TestExecution};
use anyhow::Error;
use std::collections::BTreeMap;
use std::path::Path;

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Prerequisites {
    #[serde(default)]
    asm: Vec<AsmFile>,
    #[serde(default)]
    c: Vec<CFile>,
    #[serde(default)]
    ar: Vec<ArArchive>,
    #[serde(default)]
    arch: BTreeMap<TestArch, Prerequisites>,
}

impl Prerequisites {
    pub(crate) fn build(&self, execution: &TestExecution, dest_dir: &Path) -> Result<(), Error> {
        for asm in &self.asm {
            asm.build(execution, dest_dir)?;
        }
        for c in &self.c {
            c.build(execution, dest_dir)?;
        }
        for ar in &self.ar {
            ar.build(execution, dest_dir)?;
        }
        if let Some(arch_specific) = self.arch.get(&execution.arch) {
            arch_specific.build(execution, dest_dir)?;
        }
        Ok(())
    }
}
