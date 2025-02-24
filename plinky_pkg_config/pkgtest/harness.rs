#![feature(test)]

extern crate test;

use anyhow::Error;
use insta::{assert_snapshot, with_settings};
use plinky_pkg_config::PkgConfig;
use std::fs::read_to_string;
use std::path::{Path, PathBuf};
use std::process::exit;
use test::{test_main, ShouldPanic, TestDesc, TestDescAndFn, TestFn, TestName, TestType};
use plinky_utils::posix_shell_quote;

fn test(path: PathBuf) -> Result<(), Error> {
    let rendered = match PkgConfig::parse(&read_to_string(&path)?) {
        Ok(parsed) => {
            let PkgConfig {
                name,
                description,
                url,
                version,
                requires,
                requires_private,
                conflicts,
                cflags,
                libs,
                libs_private,
            } = parsed;

            let mut output = "Successfully parsed the file!\n\n".to_string();

            let mut push_str = |name: &str, slot: Option<String>| {
                if let Some(value) = slot {
                    output.push_str(&format!("{name}: {value}\n"));
                }
            };
            push_str("Name", name);
            push_str("Description", description);
            push_str("URL", url);
            push_str("Version", version);
            push_str("Requires", requires);
            push_str("Requires.private", requires_private);
            push_str("Conflicts", conflicts);

            let mut push_args = |name: &str, slot: Option<Vec<String>>| {
                if let Some(value) = slot {
                    output.push_str(name);
                    output.push_str(": ");
                    let mut first = true;
                    for arg in value {
                        if first {
                            first = false;
                        } else {
                            output.push(' ');
                        }
                        output.push_str(&posix_shell_quote(&arg));
                    }
                    output.push('\n');
                }
            };
            push_args("CFlags", cflags);
            push_args("Libs", libs);
            push_args("Libs.private", libs_private);

            output
        }
        Err(err) => {
            format!("Failed to parse the file:\n\n{}", format_error(err.into()))
        }
    };

    let name = path.file_stem().unwrap().to_str().unwrap().to_string();
    with_settings!({
        prepend_module_to_snapshot => false,
        snapshot_path => "",
    }, {
        assert_snapshot!(name, rendered);
    });

    Ok(())
}

fn gather(path: &Path) -> Result<Vec<TestDescAndFn>, Error> {
    let mut tests = Vec::new();
    for file in path.read_dir()? {
        let entry = file?.path();
        let name = entry.file_name().unwrap().to_str().unwrap().to_string();

        if entry.extension().and_then(|s| s.to_str()) == Some("pc") {
            tests.push(TestDescAndFn {
                desc: TestDesc {
                    name: TestName::DynTestName(name),
                    ignore: false,
                    ignore_message: None,
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
                testfn: TestFn::DynTestFn(Box::new(move || match test(entry) {
                    Ok(()) => Ok(()),
                    Err(err) => panic!("{}", format_error(err)),
                })),
            });
        }
    }
    Ok(tests)
}

fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("pkgtest");

    match gather(&path) {
        Ok(tests) => test_main(&args, tests, None),
        Err(err) => {
            eprintln!("{}", format_error(err));
            exit(1);
        }
    }
}

fn format_error(error: Error) -> String {
    let mut repr = format!("error: {error}\n");
    let mut source = error.source();
    while let Some(inner) = source {
        repr.push_str(&format!("  cause: {inner}\n"));
        source = inner.source();
    }
    repr
}
