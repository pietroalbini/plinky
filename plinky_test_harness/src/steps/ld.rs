use crate::template::{Template, Value};
use crate::utils::{file_name, run};
use crate::{Arch, Step, TestContext};
use anyhow::Error;
use serde::Deserialize;
use std::process::Command;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub(crate) struct LdStep {
    output: Template,
    content: Vec<Template>,
    #[serde(default)]
    shared_library: bool,
}

impl Step for LdStep {
    fn run(&self, ctx: TestContext<'_>) -> Result<(), Error> {
        let dest_name = self.output.resolve(&*ctx.template)?;
        let content = self
            .content
            .iter()
            .map(|c| c.resolve(&*ctx.template))
            .collect::<Result<Vec<_>, _>>()?;

        let dest = ctx.dest.join(ctx.step_name);
        std::fs::create_dir_all(&dest)?;
        for input in &content {
            std::fs::copy(ctx.maybe_relative_to_src(&input), dest.join(file_name(input)))?;
        }

        run(Command::new("ld")
            .current_dir(&dest)
            .arg("-o")
            .arg(&dest_name)
            .args(content.iter().map(|c| file_name(c)).collect::<Vec<_>>())
            .args(if self.shared_library { &["-shared"] as &[_] } else { &[] })
            .arg("--hash-style=sysv") // FIXME: temporary until we implement GNU hash
            .args(match ctx.arch {
                Arch::X86 => ["-m", "elf_i386"],
                Arch::X86_64 => ["-m", "elf_x86_64"],
            }))?;

        ctx.template.set_variable(ctx.step_name, Value::Path(dest.join(dest_name)));

        Ok(())
    }

    fn templates(&self) -> Vec<Template> {
        self.content.iter().cloned().chain(std::iter::once(self.output.clone())).collect()
    }
}
