use crate::cli::{parse, CliError, CliInput, CliOptions, DebugPrint, DynamicLinker, Mode};
use crate::debug_print::filters::ObjectsFilter;
use std::collections::BTreeSet;

macro_rules! btreeset {
    ($($val:expr),*$(,)?) => {{
        let mut set = BTreeSet::new();
        $(set.insert($val);)*
        set
    }}
}

#[test]
fn test_no_flags() {
    assert_eq!(
        Ok(CliOptions { inputs: Vec::new(), ..default_options_static() }),
        parse(std::iter::empty::<String>())
    );
}

#[test]
fn test_one_input() {
    assert_eq!(
        Ok(CliOptions { inputs: vec![p("foo")], ..default_options_static() }),
        parse(["foo"].into_iter())
    )
}

#[test]
fn test_two_inputs() {
    assert_eq!(
        Ok(CliOptions { inputs: vec![p("foo"), p("bar")], ..default_options_static() }),
        parse(["foo", "bar"].into_iter())
    )
}

#[test]
fn test_output_flags() {
    const VARIANTS: &[&[&str]] = &[
        &["foo", "-obar"],
        &["foo", "-o", "bar"],
        &["foo", "--output=bar"],
        &["foo", "--output", "bar"],
    ];

    for flags in VARIANTS {
        assert_eq!(
            Ok(CliOptions {
                inputs: vec![p("foo")],
                output: "bar".into(),
                ..default_options_static()
            }),
            parse(flags.iter().copied())
        );
    }
}

#[test]
fn test_multiple_output_flags() {
    const VARIANTS: &[(&str, &[&str])] = &[
        ("-o", &["foo", "-obar", "-obaz"]),
        ("-o", &["foo", "-o", "bar", "-o", "baz"]),
        ("--output", &["foo", "--output=bar", "--output=baz"]),
        ("--output", &["foo", "--output", "bar", "--output", "baz"]),
    ];

    for (duplicate, flags) in VARIANTS {
        assert_eq!(Err(CliError::DuplicateFlag((*duplicate).into())), parse(flags.iter().copied()));
    }
}

#[test]
fn test_entry_flags() {
    const VARIANTS: &[&[&str]] = &[
        &["foo", "-ebar"],
        &["foo", "-e", "bar"],
        &["foo", "--entry=bar"],
        &["foo", "--entry", "bar"],
    ];

    for flags in VARIANTS {
        assert_eq!(
            Ok(CliOptions {
                inputs: vec![p("foo")],
                entry: Some("bar".into()),
                ..default_options_static()
            }),
            parse(flags.iter().copied())
        );
    }
}

#[test]
fn test_multiple_entry_flags() {
    const VARIANTS: &[(&str, &[&str])] = &[
        ("-e", &["foo", "-ebar", "-ebaz"]),
        ("-e", &["foo", "-e", "bar", "-e", "baz"]),
        ("--entry", &["foo", "--entry=bar", "--entry=baz"]),
        ("--entry", &["foo", "--entry", "bar", "--entry", "baz"]),
    ];

    for (duplicate, flags) in VARIANTS {
        assert_eq!(Err(CliError::DuplicateFlag((*duplicate).into())), parse(flags.iter().copied()));
    }
}

#[test]
fn test_debug_print() {
    let variants = [
        (
            btreeset![DebugPrint::LoadedObject(ObjectsFilter::all())],
            &["foo", "--debug-print", "loaded-object"] as &[&str],
        ),
        (
            btreeset![DebugPrint::RelocatedObject(ObjectsFilter::all())],
            &["foo", "--debug-print", "relocated-object"],
        ),
        (
            btreeset![
                DebugPrint::LoadedObject(ObjectsFilter::parse("@env").unwrap()),
                DebugPrint::RelocatedObject(ObjectsFilter::all())
            ],
            &["foo", "--debug-print", "loaded-object=@env", "--debug-print=relocated-object"],
        ),
    ];
    for (expected, flags) in variants {
        assert_eq!(
            Ok(CliOptions {
                inputs: vec![p("foo")],
                debug_print: expected,
                ..default_options_static()
            }),
            parse(flags.iter().copied())
        )
    }
}

#[test]
fn test_unsupported_debug_print() {
    assert_eq!(
        Err(CliError::UnsupportedDebugPrint("foo".into())),
        parse(["input_file", "--debug-print", "foo"].into_iter())
    );
}

#[test]
fn test_duplicate_debug_print() {
    assert_eq!(
        Err(CliError::DuplicateDebugPrint("loaded-object".into())),
        parse(
            [
                "input_file",
                "--debug-print",
                "relocated-object",
                "--debug-print",
                "loaded-object",
                "--debug-print",
                "loaded-object"
            ]
            .into_iter()
        )
    );
}

#[test]
fn test_no_executable_stack_flag() {
    let args = parse(["input_file"].into_iter()).unwrap();
    assert!(!args.executable_stack);
}

#[test]
fn test_enabling_executable_stack() {
    let args = parse(["input_file", "-z", "execstack"].into_iter()).unwrap();
    assert!(args.executable_stack);
}

#[test]
fn test_disabling_executable_stack() {
    let args = parse(["input_file", "-z", "noexecstack"].into_iter()).unwrap();
    assert!(!args.executable_stack);
}

#[test]
fn test_multiple_executable_stack_flags() {
    let cases = [
        ["input_file", "-zexecstack", "-zexecstack"],
        ["input_file", "-znoexecstack", "-znoexecstack"],
        ["input_file", "-zexecstack", "-znoexecstack"],
    ];
    for case in cases {
        assert_eq!(
            Err(CliError::DuplicateFlag("-z execstack or -z noexecstack".into())),
            parse(case.into_iter())
        );
    }
}

#[test]
fn test_gc_sections() {
    assert_eq!(
        Ok(CliOptions { inputs: vec![p("foo")], gc_sections: true, ..default_options_static() }),
        parse(["foo", "--gc-sections"].into_iter())
    );
}

#[test]
fn test_duplicate_gc_sections() {
    assert_eq!(
        Err(CliError::DuplicateFlag("--gc-sections".into())),
        parse(["foo", "--gc-sections", "--gc-sections"].into_iter())
    );
}

#[test]
fn test_dynamic_linker() {
    assert_eq!(
        Ok(CliOptions {
            inputs: vec![p("foo")],
            dynamic_linker: DynamicLinker::Custom("bar".into()),
            ..default_options_pie()
        }),
        parse(["foo", "--dynamic-linker=bar", "-pie"].into_iter())
    );
}

#[test]
fn test_duplicate_dynamic_linker() {
    assert_eq!(
        Err(CliError::DuplicateFlag("--dynamic-linker".into())),
        parse(["foo", "--dynamic-linker", "bar", "--dynamic-linker=baz"].into_iter())
    );
}

#[test]
fn test_no_pie() {
    assert_eq!(
        Ok(CliOptions { inputs: vec![p("foo")], ..default_options_static() }),
        parse(["foo", "-no-pie"].into_iter())
    );
}

#[test]
fn test_pie() {
    assert_eq!(
        Ok(CliOptions { inputs: vec![p("foo")], ..default_options_pie() }),
        parse(["foo", "-pie"].into_iter())
    );
}

#[test]
fn test_shared() {
    assert_eq!(
        Ok(CliOptions { inputs: vec![p("foo")], ..default_options_shared() }),
        parse(["foo", "-shared"].into_iter())
    )
}

#[test]
fn test_duplicate_modes() {
    for case in [
        ["foo", "-no-pie", "-pie"],
        ["foo", "-pie", "-no-pie"],
        ["foo", "-shared", "-pie"],
        ["foo", "-no-pie", "-shared"],
    ] {
        assert_eq!(Err(CliError::MultipleModeChanges), parse(case.into_iter()));
    }
}

#[test]
fn test_relro() {
    assert_eq!(
        Ok(CliOptions { inputs: vec![p("foo")], read_only_got: true, ..default_options_pie() }),
        parse(["foo", "-pie", "-z", "relro"].into_iter())
    );
}

#[test]
fn test_norelro() {
    assert_eq!(
        Ok(CliOptions { inputs: vec![p("foo")], read_only_got: false, ..default_options_pie() }),
        parse(["foo", "-pie", "-z", "norelro"].into_iter())
    );
}

#[test]
fn test_relro_without_pie() {
    assert_eq!(Err(CliError::RelroOnlyForPie), parse(["foo", "-zrelro"].into_iter()));
}

#[test]
fn test_multiple_relro_flags() {
    let cases = [
        ["input_file", "-zrelro", "-zrelro"],
        ["input_file", "-znorelro", "-znorelro"],
        ["input_file", "-zrelro", "-znorelro"],
    ];
    for case in cases {
        assert_eq!(
            Err(CliError::DuplicateFlag("-z relro or -z norelro".into())),
            parse(case.into_iter())
        );
    }
}

#[test]
fn test_lazy() {
    assert_eq!(
        Ok(CliOptions {
            inputs: vec![p("foo")],
            read_only_got_plt: false,
            ..default_options_pie()
        }),
        parse(["foo", "-pie", "-z", "lazy"].into_iter())
    )
}

#[test]
fn test_now() {
    assert_eq!(
        Ok(CliOptions {
            inputs: vec![p("foo")],
            mode: Mode::PositionIndependent,
            read_only_got_plt: true,
            ..default_options_pie()
        }),
        parse(["foo", "-pie", "-z", "now"].into_iter())
    )
}

#[test]
fn test_multiple_now_flags() {
    for case in [
        ["foo", "-znow", "-znow"],
        ["foo", "-znow", "-zlazy"],
        ["foo", "-zlazy", "-znow"],
        ["foo", "-zlazy", "-zlazy"],
    ] {
        assert_eq!(
            Err(CliError::DuplicateFlag("-z now or -z lazy".into())),
            parse(case.into_iter())
        );
    }
}

#[test]
fn test_now_without_pie() {
    assert_eq!(Err(CliError::NowOnlyForPie), parse(["foo", "-znow"].into_iter()));
}

#[test]
fn test_soname_shared() {
    for case in [["foo", "-shared", "-soname=hello.so"], ["foo", "-shared", "-hhello.so"]] {
        assert_eq!(
            Ok(CliOptions {
                inputs: vec![p("foo")],
                shared_object_name: Some("hello.so".into()),
                ..default_options_shared()
            }),
            parse(case.into_iter())
        );
    }
}

#[test]
fn test_soname_static() {
    for case in [["foo", "-soname=hello.so"], ["foo", "-hhello.so"]] {
        assert_eq!(Err(CliError::UnsupportedSharedObjectName), parse(case.into_iter()));
    }
}

#[test]
fn test_soname_pie() {
    for case in [["foo", "-pie", "-soname=hello.so"], ["foo", "-pie", "-hhello.so"]] {
        assert_eq!(Err(CliError::UnsupportedSharedObjectName), parse(case.into_iter()));
    }
}

#[test]
fn test_duplicate_soname() {
    for case in [
        ["foo", "-shared", "-soname=foo", "-soname=bar"],
        ["foo", "-shared", "-soname=foo", "-hbar"],
        ["foo", "-shared", "-hfoo", "-hbar"],
    ] {
        assert_eq!(Err(CliError::DuplicateFlag("-soname or -h".into())), parse(case.into_iter()));
    }
}

#[test]
fn test_search_paths() {
    for case in [
        &["foo", "-Lbar"] as &[&str],
        &["foo", "-L", "bar"],
        &["foo", "--library-path=bar"],
        &["foo", "--library-path", "bar"],
    ] {
        assert_eq!(
            CliOptions {
                inputs: vec![p("foo")],
                search_paths: vec!["bar".into()],
                ..default_options_static()
            },
            parse(case.iter().map(|s| *s)).unwrap()
        )
    }
}

#[test]
fn test_multiple_search_paths() {
    assert_eq!(
        CliOptions {
            inputs: vec![p("foo")],
            search_paths: vec!["bar".into(), "baz/hello".into()],
            ..default_options_static()
        },
        parse(["foo", "-Lbar", "--library-path=baz/hello"].into_iter()).unwrap(),
    )
}

#[test]
fn test_search_path_flag_without_value() {
    assert_eq!(Err(CliError::MissingValueForFlag("-L".into())), parse(["foo", "-L"].into_iter()));
    assert_eq!(
        Err(CliError::MissingValueForFlag("--library-path".into())),
        parse(["foo", "--library-path"].into_iter())
    );
    assert_eq!(
        Err(CliError::MissingValueForFlag("--library-path".into())),
        parse(["foo", "--library-path", ""].into_iter())
    );
}

#[test]
fn test_sysroot_relative_search_path() {
    for case in [
        &["foo", "-L", "=/bar"] as &[&str],
        &["foo", "-L", "$SYSROOT/bar"],
        &["foo", "--library-path", "=/bar"],
        &["foo", "--library-path", "$SYSROOT/bar"],
    ] {
        assert_eq!(
            Err(CliError::UnsupportedSysrootRelativeLibraryPath),
            parse(case.iter().map(|s| *s))
        );
    }
}

#[test]
fn test_unknown_flags() {
    assert_eq!(Err(CliError::UnsupportedFlag("--foo-bar".into())), parse(["--foo-bar"].into_iter()))
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
