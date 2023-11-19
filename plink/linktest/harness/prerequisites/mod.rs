mod asm;

use crate::prerequisites::asm::AsmFile;
use crate::TestExecution;
use anyhow::Error;
use std::path::Path;

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Prerequisites {
    #[serde(default)]
    asm: Vec<AsmFile>,
}

impl Prerequisites {
    pub(crate) fn build(&self, execution: &TestExecution, dest_dir: &Path) -> Result<(), Error> {
        for asm in &self.asm {
            asm.build(execution, dest_dir)?;
        }
        Ok(())
    }
}
