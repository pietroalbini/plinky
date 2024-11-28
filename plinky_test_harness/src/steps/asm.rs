use crate::template::{Template, Value};
use crate::utils::{file_name, run};
use crate::{Arch, Step, TestContext};
use anyhow::Error;
use serde::Deserialize;
use std::process::Command;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub(crate) struct AsmStep {
    source: Template,
    arch: Option<Arch>,
    output: Option<Template>,
    #[serde(default)]
    assembler: Assembler,
    #[serde(default)]
    auxiliary_files: Vec<Template>,
    #[serde(default = "default_true")]
    emit_x86_used: bool,
}

impl Step for AsmStep {
    fn run(&self, ctx: TestContext<'_>) -> Result<(), Error> {
        let source = ctx.maybe_relative_to_src(self.source.resolve(&*ctx.template)?);
        let source_name = file_name(&source);

        let dest_name = match &self.output {
            Some(template) => template.resolve(&*ctx.template)?,
            None => file_name(&source.with_extension("o")),
        };

        let dest = ctx.dest.join(ctx.step_name);
        std::fs::create_dir_all(&dest)?;
        std::fs::copy(&source, dest.join(&source_name))?;

        for auxiliary in &self.auxiliary_files {
            let auxiliary = ctx.maybe_relative_to_src(auxiliary.resolve(&*ctx.template)?);
            std::fs::copy(&auxiliary, dest.join(file_name(&auxiliary)))?;
        }

        match self.assembler {
            Assembler::Nasm => {
                run(Command::new("nasm")
                    .current_dir(&dest)
                    .arg("-f")
                    .arg(match self.arch.unwrap_or(ctx.arch) {
                        Arch::X86 => "elf32",
                        Arch::X86_64 => "elf64",
                    })
                    .arg("-o")
                    .arg(&dest_name)
                    .arg(&source_name))?;
            }
            Assembler::Gnu => {
                // We invoke the assembler through the C compiler rather than invoking `as` directly,
                // because some of the assembly files require the C preprocessor to be exeucted.
                run(Command::new("cc")
                    .current_dir(&dest)
                    .arg("-c")
                    .arg(match self.arch.unwrap_or(ctx.arch) {
                        Arch::X86 => "-m32",
                        Arch::X86_64 => "-m64",
                    })
                    .arg(format!(
                        "-Wa,-mx86-used-note={}",
                        if self.emit_x86_used { "yes" } else { "no" }
                    ))
                    .arg("-o")
                    .arg(&dest_name)
                    .arg(&source_name))?;
            }
        }

        ctx.template.set_variable(ctx.step_name, Value::Path(dest.join(dest_name)));

        Ok(())
    }

    fn templates(&self) -> Vec<Template> {
        std::iter::once(self.source.clone()).chain(self.auxiliary_files.iter().cloned()).collect()
    }
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
enum Assembler {
    Nasm,
    #[default]
    Gnu,
}

fn default_true() -> bool {
    true
}
