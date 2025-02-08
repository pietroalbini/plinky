use crate::cli::{
    CliError, CliInput, CliInputOptions, CliInputValue, CliOptions, DebugPrint, DynamicLinker,
    HashStyle, Mode, parse,
};
use crate::debug_print::filters::ObjectsFilter;
use std::collections::BTreeSet;

#[test]
fn test_no_flags() {
    assert_parse(&[], Ok(CliOptions { inputs: Vec::new(), ..default_options_static() }));
}

#[test]
fn test_one_input() {
    assert_parse(
        &["foo.o"],
        Ok(CliOptions { inputs: vec![i("foo.o").path()], ..default_options_static() }),
    );
}

#[test]
fn test_two_inputs() {
    assert_parse(
        &["foo.o", "bar.o"],
        Ok(CliOptions {
            inputs: vec![i("foo.o").path(), i("bar.o").path()],
            ..default_options_static()
        }),
    )
}

#[test]
fn test_output_flags() {
    assert_parse_multiple(
        &[
            &["input.o", "-obar"],
            &["input.o", "-o", "bar"],
            &["input.o", "--output=bar"],
            &["input.o", "--output", "bar"],
        ],
        Ok(CliOptions { output: "bar".into(), ..default_options_static() }),
    );
}

#[test]
fn test_multiple_output_flags() {
    assert_parse_multiple(
        &[&["input.o", "-obar", "-obaz"], &["input.o", "-o", "bar", "-o", "baz"]],
        Err(CliError::DuplicateFlag("-o".into())),
    );
    assert_parse_multiple(
        &[
            &["input.o", "--output=bar", "--output=baz"],
            &["input.o", "--output", "bar", "--output", "baz"],
        ],
        Err(CliError::DuplicateFlag("--output".into())),
    );
}

#[test]
fn test_entry_flags() {
    assert_parse_multiple(
        &[
            &["input.o", "-ebar"],
            &["input.o", "-e", "bar"],
            &["input.o", "--entry=bar"],
            &["input.o", "--entry", "bar"],
        ],
        Ok(CliOptions { entry: Some("bar".into()), ..default_options_static() }),
    );
}

#[test]
fn test_multiple_entry_flags() {
    assert_reject_duplicate_multiple(&[
        &["input.o", "-ebar", "-ebaz"],
        &["input.o", "-e", "bar", "-e", "baz"],
        &["input.o", "--entry=bar", "--entry=baz"],
        &["input.o", "--entry", "bar", "--entry", "baz"],
    ]);
}

#[test]
fn test_debug_print() {
    fn with_debug_prints<const N: usize>(types: [DebugPrint; N]) -> CliOptions {
        let mut debug_print = BTreeSet::new();
        for type_ in types {
            debug_print.insert(type_);
        }
        CliOptions { debug_print, ..default_options_static() }
    }

    assert_parse(
        &["input.o", "--debug-print", "loaded-object"],
        Ok(with_debug_prints([DebugPrint::LoadedObject(ObjectsFilter::all())])),
    );
    assert_parse(
        &["input.o", "--debug-print", "relocated-object"],
        Ok(with_debug_prints([DebugPrint::RelocatedObject(ObjectsFilter::all())])),
    );
    assert_parse(
        &["input.o", "--debug-print", "loaded-object=@env", "--debug-print=relocated-object"],
        Ok(with_debug_prints([
            DebugPrint::LoadedObject(ObjectsFilter::parse("@env").unwrap()),
            DebugPrint::RelocatedObject(ObjectsFilter::all()),
        ])),
    )
}

#[test]
fn test_unsupported_debug_print() {
    assert_parse(
        &["input.o", "--debug-print", "foo"],
        Err(CliError::UnsupportedDebugPrint("foo".into())),
    );
}

#[test]
fn test_duplicate_debug_print() {
    assert_parse(
        &[
            "input.o",
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
        &["input.o"],
        Ok(CliOptions { executable_stack: false, ..default_options_static() }),
    );
}

#[test]
fn test_enabling_executable_stack() {
    assert_parse(
        &["input.o", "-z", "execstack"],
        Ok(CliOptions { executable_stack: true, ..default_options_static() }),
    );
}

#[test]
fn test_disabling_executable_stack() {
    assert_parse(
        &["input.o", "-z", "noexecstack"],
        Ok(CliOptions { executable_stack: false, ..default_options_static() }),
    );
}

#[test]
fn test_multiple_executable_stack_flags() {
    assert_reject_duplicate_multiple(&[
        &["input.o", "-zexecstack", "-zexecstack"],
        &["input.o", "-znoexecstack", "-znoexecstack"],
        &["input.o", "-zexecstack", "-znoexecstack"],
    ]);
}

#[test]
fn test_gc_sections() {
    assert_parse(
        &["input.o", "--gc-sections"],
        Ok(CliOptions { gc_sections: true, ..default_options_static() }),
    );
}

#[test]
fn test_duplicate_gc_sections() {
    assert_reject_duplicate(&["input.o", "--gc-sections", "--gc-sections"]);
}

#[test]
fn test_dynamic_linker() {
    assert_parse(
        &["input.o", "--dynamic-linker=bar", "-pie"],
        Ok(CliOptions {
            dynamic_linker: DynamicLinker::Custom("bar".into()),
            ..default_options_pie()
        }),
    );
}

#[test]
fn test_duplicate_dynamic_linker() {
    assert_reject_duplicate(&["input.o", "--dynamic-linker", "bar", "--dynamic-linker=baz"]);
}

#[test]
fn test_no_pie() {
    assert_parse(&["input.o", "-no-pie"], Ok(CliOptions { ..default_options_static() }));
}

#[test]
fn test_pie() {
    assert_parse(&["input.o", "-pie"], Ok(CliOptions { ..default_options_pie() }));
}

#[test]
fn test_shared() {
    assert_parse(&["input.o", "-shared"], Ok(CliOptions { ..default_options_shared() }))
}

#[test]
fn test_duplicate_modes() {
    assert_parse_multiple(
        &[
            &["input.o", "-no-pie", "-pie"],
            &["input.o", "-pie", "-no-pie"],
            &["input.o", "-shared", "-pie"],
            &["input.o", "-no-pie", "-shared"],
        ],
        Err(CliError::MultipleModeChanges),
    );
}

#[test]
fn test_relro() {
    assert_parse(
        &["input.o", "-pie", "-z", "relro"],
        Ok(CliOptions { read_only_got: true, ..default_options_pie() }),
    );
}

#[test]
fn test_norelro() {
    assert_parse(
        &["input.o", "-pie", "-z", "norelro"],
        Ok(CliOptions { read_only_got: false, ..default_options_pie() }),
    );
}

#[test]
fn test_relro_without_pie() {
    assert_parse(&["input.o", "-zrelro"], Err(CliError::RelroOnlyForPie));
}

#[test]
fn test_multiple_relro_flags() {
    assert_reject_duplicate_multiple(&[
        &["input.o", "-zrelro", "-zrelro"],
        &["input.o", "-znorelro", "-znorelro"],
        &["input.o", "-zrelro", "-znorelro"],
    ]);
}

#[test]
fn test_lazy() {
    assert_parse(
        &["input.o", "-pie", "-z", "lazy"],
        Ok(CliOptions { read_only_got_plt: false, ..default_options_pie() }),
    );
}

#[test]
fn test_now() {
    assert_parse(
        &["input.o", "-pie", "-z", "now"],
        Ok(CliOptions {
            mode: Mode::PositionIndependent,
            read_only_got_plt: true,
            ..default_options_pie()
        }),
    );
}

#[test]
fn test_multiple_now_flags() {
    assert_reject_duplicate_multiple(&[
        &["input.o", "-znow", "-znow"],
        &["input.o", "-znow", "-zlazy"],
        &["input.o", "-zlazy", "-znow"],
        &["input.o", "-zlazy", "-zlazy"],
    ]);
}

#[test]
fn test_now_without_pie() {
    assert_parse(&["input.o", "-znow"], Err(CliError::NowOnlyForPie));
}

#[test]
fn test_soname_shared() {
    assert_parse_multiple(
        &[
            &["input.o", "-shared", "-soname", "hello.so"],
            &["input.o", "-shared", "-soname=hello.so"],
            &["input.o", "-shared", "-hhello.so"],
        ],
        Ok(CliOptions { shared_object_name: Some("hello.so".into()), ..default_options_shared() }),
    );
}

#[test]
fn test_soname_static() {
    assert_parse_multiple(
        &[
            &["input.o", "-soname=hello.so"],
            &["input.o", "-soname", "hello.so"],
            &["input.o", "-hhello.so"],
        ],
        Err(CliError::UnsupportedSharedObjectName),
    );
}

#[test]
fn test_soname_pie() {
    assert_parse_multiple(
        &[
            &["input.o", "-pie", "-soname=hello.so"],
            &["input.o", "-pie", "-soname", "hello.so"],
            &["input.o", "-pie", "-hhello.so"],
        ],
        Err(CliError::UnsupportedSharedObjectName),
    );
}

#[test]
fn test_duplicate_soname() {
    assert_reject_duplicate_multiple(&[
        &["input.o", "-shared", "-soname=foo", "-soname=bar"],
        &["input.o", "-shared", "-soname=foo", "-hbar"],
        &["input.o", "-shared", "-hfoo", "-hbar"],
    ]);
}

#[test]
fn test_search_paths() {
    assert_parse_multiple(
        &[
            &["input.o", "-Lbar"],
            &["input.o", "-L", "bar"],
            &["input.o", "--library-path=bar"],
            &["input.o", "--library-path", "bar"],
        ],
        Ok(CliOptions { search_paths: vec!["bar".into()], ..default_options_static() }),
    );
}

#[test]
fn test_multiple_search_paths() {
    assert_parse(
        &["input.o", "-Lbar", "--library-path=baz/hello"],
        Ok(CliOptions {
            search_paths: vec!["bar".into(), "baz/hello".into()],
            ..default_options_static()
        }),
    );
}

#[test]
fn test_search_path_flag_without_value() {
    assert_parse(&["input.o", "-L"], Err(CliError::MissingValueForFlag("-L".into())));

    assert_parse_multiple(
        &[&["input.o", "--library-path"], &["input.o", "--library-path", ""]],
        Err(CliError::MissingValueForFlag("--library-path".into())),
    );
}

#[test]
fn test_sysroot_relative_search_path() {
    assert_parse_multiple(
        &[
            &["input.o", "-L", "=/bar"],
            &["input.o", "-L", "$SYSROOT/bar"],
            &["input.o", "--library-path", "=/bar"],
            &["input.o", "--library-path", "$SYSROOT/bar"],
        ],
        Err(CliError::UnsupportedSysrootRelativeLibraryPath),
    );
}

#[test]
fn test_library() {
    assert_parse_multiple(
        &[&["-lfoo"], &["-l", "foo"], &["--library", "foo"], &["--library=foo"]],
        Ok(CliOptions { inputs: vec![i("foo").lib()], ..default_options_static() }),
    );
}

#[test]
fn test_multiple_libraries() {
    assert_parse(
        &["-lfoo", "-lbar"],
        Ok(CliOptions { inputs: vec![i("foo").lib(), i("bar").lib()], ..default_options_static() }),
    );
}

#[test]
fn test_verbatim_library() {
    assert_parse_multiple(
        &[&["-l:foo.so"], &["-l", ":foo.so"], &["--library=:foo.so"], &["--library", ":foo.so"]],
        Ok(CliOptions { inputs: vec![i("foo.so").lib_verbatim()], ..default_options_static() }),
    );
}

#[test]
fn test_bstatic_bdynamic() {
    assert_parse(
        &["-lfoo", "-Bstatic", "-lbar", "-Bdynamic", "-lbaz"],
        Ok(CliOptions {
            inputs: vec![
                i("foo").search_shared_objects(true).lib(),
                i("bar").search_shared_objects(false).lib(),
                i("baz").search_shared_objects(true).lib(),
            ],
            ..default_options_static()
        }),
    );
}

#[test]
fn test_bstatic_with_space() {
    assert_parse(&["input.o", "-B", "static"], Err(CliError::UnsupportedFlag("-B".into())));
}

#[test]
fn test_bdynamic_with_space() {
    assert_parse(&["input.o", "-B", "dynamic"], Err(CliError::UnsupportedFlag("-B".into())));
}

#[test]
fn test_hash_style() {
    assert_parse(
        &["input.o", "--hash-style=gnu"],
        Ok(CliOptions { hash_style: HashStyle::Gnu, ..default_options_static() }),
    );
    assert_parse(
        &["input.o", "--hash-style", "sysv"],
        Ok(CliOptions { hash_style: HashStyle::Sysv, ..default_options_static() }),
    );
    assert_parse(
        &["input.o", "--hash-style=both"],
        Ok(CliOptions { hash_style: HashStyle::Both, ..default_options_static() }),
    );
    assert_parse(
        &["input.o"],
        Ok(CliOptions { hash_style: HashStyle::Both, ..default_options_static() }),
    );
}

#[test]
fn test_duplicate_hash_style() {
    assert_reject_duplicate(&["input.o", "--hash-style=gnu", "--hash-style", "both"]);
}

#[test]
fn test_invalid_hash_style() {
    assert_parse(
        &["input.o", "--hash-style=random"],
        Err(CliError::UnsupportedHashStyle("random".into())),
    );
}

#[test]
fn test_as_needed() {
    assert_parse(
        &["liba.so", "-lb", "--as-needed", "libc.so", "-ld", "--no-as-needed", "libe.so", "-lf"],
        Ok(CliOptions {
            inputs: vec![
                i("liba.so").as_needed(false).path(),
                i("b").as_needed(false).lib(),
                i("libc.so").as_needed(true).path(),
                i("d").as_needed(true).lib(),
                i("libe.so").as_needed(false).path(),
                i("f").as_needed(false).lib(),
            ],
            ..default_options_static()
        }),
    )
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

fn i(name: &str) -> InputBuilder {
    InputBuilder { name: name.into(), search_shared_objects: true, as_needed: false }
}

struct InputBuilder {
    name: String,
    search_shared_objects: bool,
    as_needed: bool,
}

impl InputBuilder {
    fn search_shared_objects(mut self, value: bool) -> Self {
        self.search_shared_objects = value;
        self
    }

    fn as_needed(mut self, value: bool) -> Self {
        self.as_needed = value;
        self
    }

    fn path(self) -> CliInput {
        let value = CliInputValue::Path(self.name.clone().into());
        self.finalize(value)
    }

    fn lib(self) -> CliInput {
        let value = CliInputValue::Library(self.name.clone());
        self.finalize(value)
    }

    fn lib_verbatim(self) -> CliInput {
        let value = CliInputValue::LibraryVerbatim(self.name.clone());
        self.finalize(value)
    }

    fn finalize(self, value: CliInputValue) -> CliInput {
        CliInput {
            value,
            options: CliInputOptions {
                search_shared_objects: self.search_shared_objects,
                as_needed: self.as_needed,
            },
        }
    }
}

fn default_options_static() -> CliOptions {
    CliOptions {
        inputs: vec![i("input.o").path()],
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
        hash_style: HashStyle::Both,
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
