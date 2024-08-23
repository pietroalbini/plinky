use crate::template::{Template, Value};
use crate::utils::{file_name, run};
use crate::{Arch, Step, TestContext};
use anyhow::Error;
use serde::Deserialize;
use std::process::Command;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RustStep {
    source: Template,
    #[serde(default)]
    panic: Panic,
}

impl Step for RustStep {
    fn run(&self, ctx: TestContext<'_>) -> Result<(), Error> {
        let source = ctx.maybe_relative_to_src(self.source.resolve(&*ctx.template)?);
        let source_name = file_name(&source);
        let dest_name = format!("lib{}", file_name(&source.with_extension("a")));

        let dest = ctx.dest.join(ctx.step_name);
        std::fs::create_dir_all(&dest)?;
        std::fs::copy(&source, dest.join(&source_name))?;

        run(Command::new("rustc")
            .current_dir(&dest)
            .arg("--target")
            .arg(match ctx.arch {
                Arch::X86 => "i686-unknown-linux-gnu",
                Arch::X86_64 => "x86_64-unknown-linux-gnu",
            })
            .arg("--crate-type=staticlib")
            .arg(match self.panic {
                Panic::Abort => "-Cpanic=abort",
            })
            .arg("-o")
            .arg(&dest_name)
            .arg(&source_name))?;

        ctx.template.set_variable(ctx.step_name, Value::Path(dest.join(dest_name)));

        Ok(())
    }

    fn templates(&self) -> Vec<Template> {
        vec![self.source.clone()]
    }
}

#[derive(serde::Deserialize, Debug, Default, Clone, Copy)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
enum Panic {
    #[default]
    Abort,
}
