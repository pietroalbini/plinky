use crate::template::{Template, Value};
use crate::utils::{file_name, run};
use crate::{Step, TestContext};
use anyhow::Error;
use serde::Deserialize;
use std::process::Command;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub(crate) struct ArStep {
    output: Template,
    content: Vec<Template>,
    symbol_table: Option<bool>,
}

impl Step for ArStep {
    fn run(&self, ctx: TestContext<'_>) -> Result<(), Error> {
        let dest_name = self.output.resolve(ctx.template)?;
        let content =
            self.content.iter().map(|c| c.resolve(ctx.template)).collect::<Result<Vec<_>, _>>()?;

        let dest = ctx.dest.join(ctx.step_name);
        std::fs::create_dir_all(&dest)?;
        for input in &content {
            std::fs::copy(ctx.maybe_relative_to_src(&input), dest.join(file_name(input)))?;
        }

        let mut flags = "rc".to_string();
        match self.symbol_table {
            None | Some(true) => flags.push('s'),
            Some(false) => flags.push('S'),
        }

        run(Command::new("ar")
            .current_dir(&dest)
            .arg(flags)
            .arg(&dest_name)
            .args(content.iter().map(|c| file_name(c)).collect::<Vec<_>>()))?;

        ctx.template.set_variable(ctx.step_name, Value::Path(dest.join(dest_name)));

        Ok(())
    }

    fn templates(&self) -> Vec<Template> {
        std::iter::once(self.output.clone()).chain(self.content.iter().cloned()).collect()
    }
}
