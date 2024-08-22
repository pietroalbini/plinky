use crate::template::TemplateContext;
use crate::Step;
use anyhow::{Context, Error};
use std::path::{Path, PathBuf};
use tempfile::TempDir;
use crate::utils::record_snapshot;
use std::process::Command;

#[derive(Debug)]
pub(crate) struct Test {
    pub(crate) arch: Arch,
    pub(crate) steps: Vec<TestStep>,
    pub(crate) source_dir: PathBuf,
}

impl Test {
    pub(crate) fn run(mut self) -> Result<(), Error> {
        let mut template_ctx = TemplateContext::new();
        template_ctx.set_variable("arch", &self.arch.to_string());

        // Cleanup for the temporary directory is done manually at the end, to ensure that the
        // build artifacts are present for inspection during a failure.
        let dest = TempDir::new()?.into_path();
        eprintln!("output directory: {}", dest.display());

        let mut last_number_of_completed = usize::MAX;
        loop {
            let mut number_of_completed = 0;
            for step in &mut self.steps {
                if step.completed {
                    number_of_completed += 1;
                    continue;
                }

                if step.step.templates().iter().all(|t| t.will_resolve(&template_ctx)) {
                    eprintln!("===> running step {}", step.name);
                    step.step
                        .run(TestContext {
                            step_name: &step.name,
                            src: &self.source_dir,
                            dest: &dest,
                            arch: self.arch,
                            template: &mut template_ctx,
                        })
                        .with_context(|| format!("failed to execute step {}", step.name))?;
                    step.completed = true;
                }
            }

            if number_of_completed == self.steps.len() {
                // We are done!
                break;
            } else if number_of_completed == last_number_of_completed {
                // There are either variables pointing to missing steps, or circular dependencies.
                let unmet_dependencies = self
                    .steps
                    .iter()
                    .filter(|s| !s.completed)
                    .map(|s| s.name.as_str())
                    .collect::<Vec<_>>();
                anyhow::bail!(
                    "these steps have unmet dependencies: {}",
                    unmet_dependencies.join(", ")
                );
            } else {
                last_number_of_completed = number_of_completed;
            }
        }

        std::fs::remove_dir_all(&dest).context("failed to remove the temporary directory")?;
        Ok(())
    }
}

pub struct TestContext<'a> {
    pub step_name: &'a str,
    pub src: &'a Path,
    pub dest: &'a Path,
    pub arch: Arch,
    pub template: &'a mut TemplateContext,
}

impl TestContext<'_> {
    pub fn maybe_relative_to_src(&self, path: impl AsRef<Path>) -> PathBuf {
        let path = path.as_ref();
        if path.is_absolute() {
            path.into()
        } else {
            self.src.join(path)
        }
    }

    pub fn run_and_snapshot(&self, command: &mut Command) -> Result<bool, Error> {
        let name = self.step_name.split_once('.').expect("invalid step name").1;
        let arch = match self.arch {
            Arch::X86 => "32bit",
            Arch::X86_64 => "64bit",
        };

        record_snapshot(&format!("{name}-{arch}"), &self.src, "command", command)
    }
}

#[derive(Debug)]
pub(crate) struct TestStep {
    name: String,
    step: Box<dyn Step>,
    completed: bool,
}

impl TestStep {
    pub(crate) fn new(name: &str, step: Box<dyn Step>) -> Self {
        Self { name: name.into(), step, completed: false }
    }
}

#[derive(Debug, Clone, Copy, serde::Deserialize)]
pub enum Arch {
    #[serde(rename = "x86")]
    X86,
    #[serde(rename = "x86_64")]
    X86_64,
}

impl std::fmt::Display for Arch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Arch::X86 => f.write_str("x86"),
            Arch::X86_64 => f.write_str("x86_64"),
        }
    }
}
