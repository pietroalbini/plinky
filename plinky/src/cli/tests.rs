use crate::cli::{parse, CliError, CliInput, CliOptions, DebugPrint, DynamicLinker, Mode};
use crate::debug_print::filters::ObjectsFilter;
use std::collections::BTreeSet;

#[test]
fn test_no_flags() {
    assert_parse(&[], Ok(CliOptions { inputs: Vec::new(), ..default_options_static() }));
}

#[test]
fn test_one_input() {
    assert_parse(&["foo"], Ok(CliOptions { inputs: vec![p("foo")], ..default_options_static() }));
}

#[test]
fn test_two_inputs() {
    assert_parse(
        &["foo", "bar"],
        Ok(CliOptions { inputs: vec![p("foo"), p("bar")], ..default_options_static() }),
    )
}

#[test]
fn test_output_flags() {
    assert_parse_multiple(
        &[
            &["foo", "-obar"],
            &["foo", "-o", "bar"],
            &["foo", "--output=bar"],
            &["foo", "--output", "bar"],
        ],
        Ok(CliOptions { inputs: vec![p("foo")], output: "bar".into(), ..default_options_static() }),
    );
}

#[test]
fn test_multiple_output_flags() {
    assert_parse_multiple(
        &[&["foo", "-obar", "-obaz"], &["foo", "-o", "bar", "-o", "baz"]],
        Err(CliError::DuplicateFlag("-o".into())),
    );
    assert_parse_multiple(
        &[&["foo", "--output=bar", "--output=baz"], &["foo", "--output", "bar", "--output", "baz"]],
        Err(CliError::DuplicateFlag("--output".into())),
    );
}

#[test]
fn test_entry_flags() {
    assert_parse_multiple(
        &[
            &["foo", "-ebar"],
            &["foo", "-e", "bar"],
            &["foo", "--entry=bar"],
            &["foo", "--entry", "bar"],
        ],
        Ok(CliOptions {
            inputs: vec![p("foo")],
            entry: Some("bar".into()),
            ..default_options_static()
        }),
    );
}

#[test]
fn test_multiple_entry_flags() {
    assert_reject_duplicate_multiple(&[
        &["foo", "-ebar", "-ebaz"],
        &["foo", "-e", "bar", "-e", "baz"],
        &["foo", "--entry=bar", "--entry=baz"],
        &["foo", "--entry", "bar", "--entry", "baz"],
    ]);
}

#[test]
fn test_debug_print() {
    fn with_debug_prints<const N: usize>(types: [DebugPrint; N]) -> CliOptions {
        let mut debug_print = BTreeSet::new();
        for type_ in types {
            debug_print.insert(type_);
        }
        CliOptions { inputs: vec![p("foo")], debug_print, ..default_options_static() }
    }

    assert_parse(
        &["foo", "--debug-print", "loaded-object"],
        Ok(with_debug_prints([DebugPrint::LoadedObject(ObjectsFilter::all())])),
    );
    assert_parse(
        &["foo", "--debug-print", "relocated-object"],
        Ok(with_debug_prints([DebugPrint::RelocatedObject(ObjectsFilter::all())])),
    );
    assert_parse(
        &["foo", "--debug-print", "loaded-object=@env", "--debug-print=relocated-object"],
        Ok(with_debug_prints([
            DebugPrint::LoadedObject(ObjectsFilter::parse("@env").unwrap()),
            DebugPrint::RelocatedObject(ObjectsFilter::all()),
        ])),
    )
}

#[test]
fn test_unsupported_debug_print() {
    assert_parse(
        &["input_file", "--debug-print", "foo"],
        Err(CliError::UnsupportedDebugPrint("foo".into())),
    );
}

#[test]
fn test_duplicate_debug_print() {
    assert_parse(
        &[
            "input_file",
            "--debug-print",
            "relocated-object",
            "--debug-print",
            "loaded-object",
            "--debug-print",
            "loaded-object",
        ],
        Err(CliError::DuplicateDebugPrint("loaded-object".into())),
    );
}

#[test]
fn test_no_executable_stack_flag() {
    assert_parse(
        &["foo"],
        Ok(CliOptions {
            inputs: vec![p("foo")],
            executable_stack: false,
            ..default_options_static()
        }),
    );
}

#[test]
fn test_enabling_executable_stack() {
    assert_parse(
        &["foo", "-z", "execstack"],
        Ok(CliOptions {
            inputs: vec![p("foo")],
            executable_stack: true,
            ..default_options_static()
        }),
    );
}

#[test]
fn test_disabling_executable_stack() {
    assert_parse(
        &["foo", "-z", "noexecstack"],
        Ok(CliOptions {
            inputs: vec![p("foo")],
            executable_stack: false,
            ..default_options_static()
        }),
    );
}

#[test]
fn test_multiple_executable_stack_flags() {
    assert_reject_duplicate_multiple(&[
        &["input_file", "-zexecstack", "-zexecstack"],
        &["input_file", "-znoexecstack", "-znoexecstack"],
        &["input_file", "-zexecstack", "-znoexecstack"],
    ]);
}

#[test]
fn test_gc_sections() {
    assert_parse(
        &["foo", "--gc-sections"],
        Ok(CliOptions { inputs: vec![p("foo")], gc_sections: true, ..default_options_static() }),
    );
}

#[test]
fn test_duplicate_gc_sections() {
    assert_reject_duplicate(&["foo", "--gc-sections", "--gc-sections"]);
}

#[test]
fn test_dynamic_linker() {
    assert_parse(
        &["foo", "--dynamic-linker=bar", "-pie"],
        Ok(CliOptions {
            inputs: vec![p("foo")],
            dynamic_linker: DynamicLinker::Custom("bar".into()),
            ..default_options_pie()
        }),
    );
}

#[test]
fn test_duplicate_dynamic_linker() {
    assert_reject_duplicate(&["foo", "--dynamic-linker", "bar", "--dynamic-linker=baz"]);
}

#[test]
fn test_no_pie() {
    assert_parse(
        &["foo", "-no-pie"],
        Ok(CliOptions { inputs: vec![p("foo")], ..default_options_static() }),
    );
}

#[test]
fn test_pie() {
    assert_parse(
        &["foo", "-pie"],
        Ok(CliOptions { inputs: vec![p("foo")], ..default_options_pie() }),
    );
}

#[test]
fn test_shared() {
    assert_parse(
        &["foo", "-shared"],
        Ok(CliOptions { inputs: vec![p("foo")], ..default_options_shared() }),
    )
}

#[test]
fn test_duplicate_modes() {
    assert_parse_multiple(
        &[
            &["foo", "-no-pie", "-pie"],
            &["foo", "-pie", "-no-pie"],
            &["foo", "-shared", "-pie"],
            &["foo", "-no-pie", "-shared"],
        ],
        Err(CliError::MultipleModeChanges),
    );
}

#[test]
fn test_relro() {
    assert_parse(
        &["foo", "-pie", "-z", "relro"],
        Ok(CliOptions { inputs: vec![p("foo")], read_only_got: true, ..default_options_pie() }),
    );
}

#[test]
fn test_norelro() {
    assert_parse(
        &["foo", "-pie", "-z", "norelro"],
        Ok(CliOptions { inputs: vec![p("foo")], read_only_got: false, ..default_options_pie() }),
    );
}

#[test]
fn test_relro_without_pie() {
    assert_parse(&["foo", "-zrelro"], Err(CliError::RelroOnlyForPie));
}

#[test]
fn test_multiple_relro_flags() {
    assert_reject_duplicate_multiple(&[
        &["input_file", "-zrelro", "-zrelro"],
        &["input_file", "-znorelro", "-znorelro"],
        &["input_file", "-zrelro", "-znorelro"],
    ]);
}

#[test]
fn test_lazy() {
    assert_parse(
        &["foo", "-pie", "-z", "lazy"],
        Ok(CliOptions {
            inputs: vec![p("foo")],
            read_only_got_plt: false,
            ..default_options_pie()
        }),
    );
}

#[test]
fn test_now() {
    assert_parse(
        &["foo", "-pie", "-z", "now"],
        Ok(CliOptions {
            inputs: vec![p("foo")],
            mode: Mode::PositionIndependent,
            read_only_got_plt: true,
            ..default_options_pie()
        }),
    );
}

#[test]
fn test_multiple_now_flags() {
    assert_reject_duplicate_multiple(&[
        &["foo", "-znow", "-znow"],
        &["foo", "-znow", "-zlazy"],
        &["foo", "-zlazy", "-znow"],
        &["foo", "-zlazy", "-zlazy"],
    ]);
}

#[test]
fn test_now_without_pie() {
    assert_parse(&["foo", "-znow"], Err(CliError::NowOnlyForPie));
}

#[test]
fn test_soname_shared() {
    assert_parse_multiple(
        &[
            &["foo", "-shared", "-soname", "hello.so"],
            &["foo", "-shared", "-soname=hello.so"],
            &["foo", "-shared", "-hhello.so"],
        ],
        Ok(CliOptions {
            inputs: vec![p("foo")],
            shared_object_name: Some("hello.so".into()),
            ..default_options_shared()
        }),
    );
}

#[test]
fn test_soname_static() {
    assert_parse_multiple(
        &[&["foo", "-soname=hello.so"], &["foo", "-soname", "hello.so"], &["foo", "-hhello.so"]],
        Err(CliError::UnsupportedSharedObjectName),
    );
}

#[test]
fn test_soname_pie() {
    assert_parse_multiple(
        &[
            &["foo", "-pie", "-soname=hello.so"],
            &["foo", "-pie", "-soname", "hello.so"],
            &["foo", "-pie", "-hhello.so"],
        ],
        Err(CliError::UnsupportedSharedObjectName),
    );
}

#[test]
fn test_duplicate_soname() {
    assert_reject_duplicate_multiple(&[
        &["foo", "-shared", "-soname=foo", "-soname=bar"],
        &["foo", "-shared", "-soname=foo", "-hbar"],
        &["foo", "-shared", "-hfoo", "-hbar"],
    ]);
}

#[test]
fn test_search_paths() {
    assert_parse_multiple(
        &[
            &["foo", "-Lbar"],
            &["foo", "-L", "bar"],
            &["foo", "--library-path=bar"],
            &["foo", "--library-path", "bar"],
        ],
        Ok(CliOptions {
            inputs: vec![p("foo")],
            search_paths: vec!["bar".into()],
            ..default_options_static()
        }),
    );
}

#[test]
fn test_multiple_search_paths() {
    assert_parse(
        &["foo", "-Lbar", "--library-path=baz/hello"],
        Ok(CliOptions {
            inputs: vec![p("foo")],
            search_paths: vec!["bar".into(), "baz/hello".into()],
            ..default_options_static()
        }),
    );
}

#[test]
fn test_search_path_flag_without_value() {
    assert_parse(&["foo", "-L"], Err(CliError::MissingValueForFlag("-L".into())));

    assert_parse_multiple(
        &[&["foo", "--library-path"], &["foo", "--library-path", ""]],
        Err(CliError::MissingValueForFlag("--library-path".into())),
    );
}

#[test]
fn test_sysroot_relative_search_path() {
    assert_parse_multiple(
        &[
            &["foo", "-L", "=/bar"],
            &["foo", "-L", "$SYSROOT/bar"],
            &["foo", "--library-path", "=/bar"],
            &["foo", "--library-path", "$SYSROOT/bar"],
        ],
        Err(CliError::UnsupportedSysrootRelativeLibraryPath),
    );
}

#[test]
fn test_unknown_flags() {
    assert_parse(&["--foo-bar"], Err(CliError::UnsupportedFlag("--foo-bar".into())));
}

#[track_caller]
fn assert_reject_duplicate(case: &[&str]) {
    assert_reject_duplicate_multiple(&[case])
}

#[track_caller]
fn assert_reject_duplicate_multiple(cases: &[&[&str]]) {
    assert!(!cases.is_empty(), "no cases provided");
    for case in cases {
        let parsed = parse(case.iter().copied());
        assert!(
            matches!(parsed, Err(CliError::DuplicateFlag(_))),
            "parsing {case:?} returned {parsed:?}"
        );
    }
}

#[track_caller]
fn assert_parse(case: &[&str], expected: Result<CliOptions, CliError>) {
    assert_parse_multiple(&[case], expected)
}

#[track_caller]
fn assert_parse_multiple(cases: &[&[&str]], expected: Result<CliOptions, CliError>) {
    assert!(!cases.is_empty(), "no cases provided");
    for case in cases {
        let parsed = parse(case.iter().copied());
        assert_eq!(expected, parsed, "parsing {case:?}");
    }
}

fn p(name: &str) -> CliInput {
    CliInput::Path(name.into())
}

fn default_options_static() -> CliOptions {
    CliOptions {
        inputs: Vec::new(),
        output: "a.out".into(),
        entry: Some("_start".into()),
        gc_sections: false,
        debug_print: BTreeSet::new(),
        executable_stack: false,
        read_only_got: false,
        read_only_got_plt: false,
        dynamic_linker: DynamicLinker::PlatformDefault,
        search_paths: Vec::new(),
        shared_object_name: None,
        mode: Mode::PositionDependent,
    }
}

fn default_options_pie() -> CliOptions {
    CliOptions {
        mode: Mode::PositionIndependent,
        dynamic_linker: DynamicLinker::PlatformDefault,
        ..default_options_static()
    }
}

fn default_options_shared() -> CliOptions {
    CliOptions { mode: Mode::SharedLibrary, entry: None, ..default_options_static() }
}
