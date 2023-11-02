use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=tests/linktest/");

    let mut tests_list = String::new();
    for test in std::fs::read_dir("tests/linktest/").unwrap() {
        let test = test.unwrap().path();
        if !test.is_dir() {
            continue;
        }

        let test_name = test.file_name().unwrap().to_str().unwrap().to_string();
        let mut files = Vec::new();
        for file in std::fs::read_dir(&test).unwrap() {
            let file = file.unwrap().path();
            if file.is_dir() {
                panic!("dir {file:?} inside of test");
            }
            files.push(std::fs::canonicalize(file).unwrap().to_str().unwrap().to_string());
        }

        let files = files
            .iter()
            .map(|s| format!("\"{s}\""))
            .collect::<Vec<_>>()
            .join(",");
        tests_list.push_str(&format!("linktest! {{ {test_name}, files[{files}] }}\n"));
    }

    std::fs::write(
        PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("linktest_definition.rs"),
        tests_list.as_bytes(),
    )
    .unwrap();
}
