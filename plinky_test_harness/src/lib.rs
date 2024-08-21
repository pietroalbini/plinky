#![feature(test)]

extern crate test;

pub mod prerequisites;
pub mod template;
pub mod utils;

use crate::utils::err_str;
use anyhow::Error;
use std::path::Path;
use test::{ShouldPanic, TestDesc, TestDescAndFn, TestFn, TestName, TestType};

pub trait TestGatherer {
    const MANIFEST_NAME: &'static str;
    fn tests_for_file(&self, path: &Path) -> Result<Vec<Test>, Error>;
}

pub struct Test {
    pub name: String,
    pub exec: Box<dyn FnOnce() -> Result<(), Error> + Send>,
    pub ignore: Option<String>,
}

pub fn main<T: TestGatherer>(path: &Path, gatherer: T) {
    let args = std::env::args().collect::<Vec<_>>();
    test::test_main(&args, err_str(gather_tests(path, gatherer)).unwrap(), None)
}

fn gather_tests<T: TestGatherer>(path: &Path, gatherer: T) -> Result<Vec<TestDescAndFn>, Error> {
    let mut tests = Vec::new();
    for entry in path.read_dir()? {
        let path = entry?.path();

        let manifest_path = path.join(T::MANIFEST_NAME);
        if !manifest_path.exists() {
            continue;
        }

        for test in gatherer.tests_for_file(&manifest_path)? {
            tests.push(TestDescAndFn {
                desc: TestDesc {
                    name: TestName::DynTestName(test.name),
                    ignore: test.ignore.is_some(),
                    ignore_message: test.ignore.map(leak),
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
                testfn: TestFn::DynTestFn(Box::new(move || err_str((test.exec)()))),
            })
        }
    }
    Ok(tests)
}

fn leak(string: String) -> &'static str {
    Box::leak(Box::new(string)).as_str()
}
