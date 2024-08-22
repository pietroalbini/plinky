use crate::template::Template;
use crate::utils::{file_name, run};
use crate::{Arch, Step, TestContext};
use anyhow::Error;
use serde::Deserialize;
use std::process::Command;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AsmStep {
    source: Template,
}

impl Step for AsmStep {
    fn run(&self, ctx: TestContext<'_>) -> Result<(), Error> {
        let source = ctx.maybe_relative_to_src(self.source.resolve(&ctx.template)?);
        let source_name = file_name(&source);
        let dest_name = file_name(&source.with_extension("o"));

        let dest = ctx.dest.join(ctx.step_name);
        std::fs::create_dir_all(&dest)?;
        std::fs::copy(&source, dest.join(&source_name))?;

        run(Command::new("as")
            .current_dir(&dest)
            .arg(match ctx.arch {
                Arch::X86 => "--32",
                Arch::X86_64 => "--64",
            })
            .arg("-o")
            .arg(&dest_name)
            .arg(&source_name))?;

        ctx.template.set_variable(ctx.step_name, dest.join(dest_name).to_str().unwrap());

        Ok(())
    }

    fn templates(&self) -> Vec<Template> {
        vec![self.source.clone()]
    }
}
