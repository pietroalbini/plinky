use crate::tests::{Arch, Test, TestStep};
use crate::utils::err_str;
use crate::Step;
use anyhow::{Context, Error};
use serde::de::DeserializeOwned;
use std::collections::BTreeMap;
use std::path::Path;
use test::{ShouldPanic, TestDesc, TestDescAndFn, TestFn, TestName, TestType};
use toml::Value;

pub(crate) fn gather(
    path: &Path,
    prefix: &str,
    define_steps: DefineStepsFn,
) -> Result<Vec<TestDescAndFn>, Error> {
    let mut tests = Vec::new();

    for entry in path.read_dir()? {
        let entry = entry?.path();

        let toml = entry.join("test.toml");
        if toml.is_file() {
            create_tests(&mut tests, prefix, &toml, define_steps)
                .with_context(|| format!("failed to create tests from {}", toml.display()))?;
        } else if entry.is_dir() {
            let prefix = format!("{}{}/", prefix, entry.file_name().unwrap().to_str().unwrap());
            tests.extend(gather(&entry, &prefix, define_steps)?);
        }
    }

    Ok(tests)
}

fn create_tests(
    tests: &mut Vec<TestDescAndFn>,
    prefix: &str,
    toml_path: &Path,
    define_steps: DefineStepsFn,
) -> Result<(), Error> {
    let source_dir = toml_path.parent().unwrap();
    let name = format!("{}{}", prefix, source_dir.file_name().unwrap().to_str().unwrap());

    let raw = std::fs::read_to_string(toml_path)?;
    let toml: Toml = toml::from_str(&raw)?;

    for &arch in &toml.archs {
        let mut definer = DefineSteps {
            undefined: toml.steps.clone(),
            defined: Vec::new(),
            defined_leafs: Vec::new(),
        };
        define_steps(&mut definer)?;

        let missing_step_kinds = definer.undefined.into_keys().collect::<Vec<_>>();
        if !missing_step_kinds.is_empty() {
            anyhow::bail!(
                "test contains the following undefined step types: {}",
                missing_step_kinds.join(", ")
            );
        }

        for leaf in &definer.defined_leafs {
            let mut steps = definer.defined.clone();
            steps.push(leaf.clone());

            let leaf_name = if definer.defined_leafs.len() > 1 {
                let name = leaf.name.split_once('.').expect("bad step name").1;
                format!("{name}, ")
            } else {
                String::new()
            };

            let test = Test { arch, steps, source_dir: source_dir.into() };

            tests.push(TestDescAndFn {
                desc: TestDesc {
                    name: TestName::DynTestName(format!("{name} ({leaf_name}{arch})")),
                    ignore: toml.ignore.is_some(),
                    ignore_message: toml.ignore.clone().map(leak),
                    source_file: "",
                    start_line: 0,
                    start_col: 0,
                    end_line: 0,
                    end_col: 0,
                    should_panic: ShouldPanic::No,
                    compile_fail: false,
                    no_run: false,
                    test_type: TestType::IntegrationTest,
                },
                testfn: TestFn::DynTestFn(Box::new(move || err_str(test.run()))),
            });
        }
    }

    Ok(())
}

#[derive(serde::Deserialize)]
struct Toml {
    archs: Vec<Arch>,
    #[serde(default)]
    ignore: Option<String>,
    #[serde(flatten)]
    steps: BTreeMap<String, BTreeMap<String, Value>>,
}

pub(crate) type DefineStepsFn = fn(&mut DefineSteps) -> Result<&mut DefineSteps, Error>;

pub struct DefineSteps {
    undefined: BTreeMap<String, BTreeMap<String, Value>>,
    defined: Vec<TestStep>,
    defined_leafs: Vec<TestStep>,
}

impl DefineSteps {
    pub fn define_builtins(&mut self) -> Result<&mut Self, Error> {
        self.define::<crate::steps::asm::AsmStep>("asm")?
            .define::<crate::steps::ld::LdStep>("ld")?
            .define::<crate::steps::c::CStep>("c")?
            .define::<crate::steps::rust::RustStep>("rust")?
            .define::<crate::steps::ar::ArStep>("ar")?
            .define::<crate::steps::rename::RenameStep>("rename")
    }

    // Deserializing the Value into the concrete type cannot be done through dynamic dispatching.
    // This approach is similar to the one proposed in libcore for Error's provider API, where this
    // method is invoked for every set of steps to process.
    pub fn define<S: Step + DeserializeOwned + 'static>(
        &mut self,
        kind_name: &str,
    ) -> Result<&mut Self, Error> {
        if let Some(steps) = self.undefined.remove(kind_name) {
            for (step_name, data) in steps {
                let name = format!("{kind_name}.{step_name}");
                let step = Box::new(
                    data.try_into::<S>().with_context(|| format!("failed to parse step {name}"))?,
                );
                if step.is_leaf() {
                    self.defined_leafs.push(TestStep::new(&name, step));
                } else {
                    self.defined.push(TestStep::new(&name, step));
                }
            }
        }
        Ok(self)
    }
}

fn leak(string: String) -> &'static str {
    Box::leak(Box::new(string)).as_str()
}
