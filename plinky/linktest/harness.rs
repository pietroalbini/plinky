use anyhow::{anyhow, bail, Error};
use plinky_test_harness::template::{
    Template, TemplateContext, TemplateContextGetters, TemplateFunction, Value,
};
use plinky_test_harness::utils::RunAndSnapshot;
use plinky_test_harness::{Step, TestContext};
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::iter::once;
use std::path::Path;
use std::process::Command;

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
struct PlinkyStep {
    cmd: Vec<Template>,
    kind: Template,
    #[serde(default)]
    debug_print: Vec<String>,
    #[serde(default)]
    run_env: BTreeMap<String, String>,
}

impl Step for PlinkyStep {
    fn run(&self, ctx: TestContext<'_>) -> Result<(), Error> {
        let mut runner = ctx.run_and_snapshot();
        let (res, err) = match self.kind.resolve(ctx.template)?.as_str() {
            "link-fail" => {
                (!self.link(&ctx, &mut runner)?, "linking was supposed to fail but passed!")
            }
            "link-pass" => {
                (self.link(&ctx, &mut runner)?, "linking was supposed to pass but failed!")
            }
            "run-fail" => {
                (!self.run(&ctx, &mut runner)?, "running was supposed to fail but passed!")
            }
            "run-pass" => {
                (self.run(&ctx, &mut runner)?, "running was supposed to pass but failed!")
            }
            kind => bail!("unsupported test kind: {kind}"),
        };
        runner.persist();

        if !res {
            bail!("{err}");
        }
        Ok(())
    }

    fn templates(&self) -> Vec<Template> {
        once(self.kind.clone()).chain(self.cmd.iter().cloned()).collect()
    }

    fn is_leaf(&self) -> bool {
        true
    }
}

impl PlinkyStep {
    fn link(&self, ctx: &TestContext<'_>, runner: &mut RunAndSnapshot) -> Result<bool, Error> {
        let dest = ctx.dest.join(ctx.step_name);
        std::fs::create_dir_all(&dest)?;

        let resolver = CopyFilesTemplateResolver { dest: &dest, context: ctx.template };
        let cmd = self.cmd.iter().map(|c| c.resolve(&resolver)).collect::<Result<Vec<_>, _>>()?;

        let mut command = Command::new(env!("CARGO_BIN_EXE_ld.plinky"));
        command.current_dir(&dest).args(&cmd).env("RUST_BACKTRACE", "1");
        for debug_print in &self.debug_print {
            command.args(["--debug-print", debug_print]);
        }

        // In NixOS, the default linker is just a stub that errors out (since you are not supposed
        // to use dynamicly linked programs built outside of Nix). We thus need to set the correct
        // linker for it, which is provided by flake.nix through the environment variable.
        let dynamic_linker_var = match &ctx.arch {
            plinky_test_harness::Arch::X86 => "PLINKY_TEST_DYNAMIC_LINKER_32",
            plinky_test_harness::Arch::X86_64 => "PLINKY_TEST_DYNAMIC_LINKER_64",
        };
        command.arg("--dynamic-linker").arg(
            std::env::var_os(dynamic_linker_var)
                .ok_or_else(|| anyhow!("missing environment variable {dynamic_linker_var}"))?,
        );

        runner.run("linking", &mut command)
    }

    fn run(&self, ctx: &TestContext<'_>, runner: &mut RunAndSnapshot) -> Result<bool, Error> {
        if !self.link(ctx, runner)? {
            runner.note("error: could not execute the program due to linking failing");
            return Ok(false);
        }

        let dest = ctx.dest.join(ctx.step_name);

        let mut command = Command::new(dest.join("a.out"));
        command.current_dir(&dest);
        for (key, value) in &self.run_env {
            command.env(key, value);
        }

        runner.run("running", &mut command)
    }
}

struct CopyFilesTemplateResolver<'a> {
    dest: &'a Path,
    context: &'a TemplateContext,
}

impl TemplateContextGetters for CopyFilesTemplateResolver<'_> {
    fn get_variable(&self, key: &str) -> Option<Cow<'_, Value>> {
        let parent = self.context.get_variable(key)?;
        match parent.as_ref() {
            Value::Path(path) => {
                let file_name = path.file_name().expect("path without file name");
                let dest = self.dest.join(file_name);
                if !dest.exists() {
                    std::fs::copy(path, self.dest.join(file_name)).expect("failed to copy file");
                }
                Some(Cow::Owned(Value::Path(file_name.into())))
            }
            _ => Some(parent),
        }
    }

    fn get_function(&self, name: &str) -> Option<&dyn TemplateFunction> {
        self.context.get_function(name)
    }
}

fn main() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("linktest");
    plinky_test_harness::main(&path, |definer| {
        definer.define_builtins()?.define::<PlinkyStep>("plinky")
    });
}
