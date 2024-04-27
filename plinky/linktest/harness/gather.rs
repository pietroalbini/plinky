use crate::tests::{TestArch, TestExecution, TestSettings};
use crate::utils::err_str;
use anyhow::Error;
use std::path::Path;
use test::{ShouldPanic, TestDesc, TestDescAndFn, TestFn, TestName, TestType};

pub(crate) fn gather_tests(path: &Path) -> Result<Vec<TestDescAndFn>, Error> {
    let mut tests = Vec::new();
    for entry in path.read_dir()? {
        let path = entry?.path();

        let test_toml_path = path.join("test.toml");
        let test_toml: TestSettings = match std::fs::read_to_string(&test_toml_path) {
            Ok(string) => toml::from_str(&string)?,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => continue,
            Err(err) => return Err(err.into()),
        };

        for arch in &test_toml.archs {
            let name = path.file_name().unwrap().to_string_lossy();
            let arch_name = match arch {
                TestArch::X86 => "x86",
                TestArch::X86_64 => "x86-64",
            };

            let execution = TestExecution::new(path.clone(), test_toml.clone(), *arch)?;
            tests.push(TestDescAndFn {
                desc: TestDesc {
                    name: TestName::DynTestName(format!("{name} ({arch_name})")),
                    ignore: test_toml.ignore.is_some(),
                    ignore_message: test_toml.ignore.clone().map(leak),
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
                testfn: TestFn::DynTestFn(Box::new(move || err_str(execution.run()))),
            })
        }
    }
    Ok(tests)
}

fn leak(string: String) -> &'static str {
    Box::leak(Box::new(string)).as_str()
}
