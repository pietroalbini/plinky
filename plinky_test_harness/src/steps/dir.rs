use crate::Step;
use crate::template::{Template, Value};
use crate::utils::file_name;
use anyhow::Error;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DirStep {
    files: Vec<Template>,
}

impl Step for DirStep {
    fn run(&self, ctx: crate::TestContext<'_>) -> Result<(), Error> {
        let dest = ctx.dest.join(ctx.step_name);
        std::fs::create_dir_all(&dest)?;

        for template in &self.files {
            let resolved = template.resolve(&*ctx.template)?;
            std::fs::copy(&resolved, dest.join(file_name(&resolved)))?;
        }

        ctx.template.set_variable(ctx.step_name, Value::Path(dest));

        Ok(())
    }

    fn templates(&self) -> Vec<Template> {
        self.files.clone()
    }
}
