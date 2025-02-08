mod lexer;
mod parser;
#[cfg(test)]
mod tests;

pub(crate) use crate::cli::parser::parse;
use crate::debug_print::filters::{ObjectsFilter, ObjectsFilterParseError};
use crate::interner::Interned;
use plinky_diagnostics::DiagnosticContext;
use plinky_elf::render_elf::{RenderElfFilters, RenderElfFiltersParseError};
use plinky_macros::{Display, Error};
use std::collections::BTreeSet;
use std::path::PathBuf;

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct CliOptions {
    pub(crate) inputs: Vec<CliInput>,
    pub(crate) output: PathBuf,
    pub(crate) entry: EntryPoint,
    pub(crate) gc_sections: bool,
    pub(crate) debug_print: BTreeSet<DebugPrint>,
    pub(crate) executable_stack: bool,
    pub(crate) read_only_got: bool,
    pub(crate) read_only_got_plt: bool,
    pub(crate) dynamic_linker: DynamicLinker,
    pub(crate) search_paths: Vec<PathBuf>,
    pub(crate) shared_object_name: Option<String>,
    pub(crate) hash_style: HashStyle,
    pub(crate) mode: Mode,
}

impl DiagnosticContext for CliOptions {}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum EntryPoint {
    None,
    Default,
    Custom(Interned<String>),
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub(crate) enum Mode {
    PositionDependent,
    PositionIndependent,
    SharedLibrary,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum DynamicLinker {
    PlatformDefault,
    Custom(String),
}

#[derive(Debug, PartialEq, Eq, Clone, PartialOrd, Ord)]
pub(crate) enum DebugPrint {
    LoadedObject(ObjectsFilter),
    Gc,
    RelocationsAnalysis,
    RelocatedObject(ObjectsFilter),
    Layout,
    FinalElf(RenderElfFilters),
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct CliInput {
    pub(crate) value: CliInputValue,
    pub(crate) options: CliInputOptions,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum CliInputValue {
    Path(PathBuf),
    Library(String),
    LibraryVerbatim(String),
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub(crate) struct CliInputOptions {
    pub(crate) search_shared_objects: bool,
    pub(crate) as_needed: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum HashStyle {
    Sysv,
    Gnu,
    Both,
}

impl HashStyle {
    pub(crate) fn has_sysv(&self) -> bool {
        match self {
            HashStyle::Sysv => true,
            HashStyle::Gnu => false,
            HashStyle::Both => true,
        }
    }

    pub(crate) fn has_gnu(&self) -> bool {
        match self {
            HashStyle::Sysv => false,
            HashStyle::Gnu => true,
            HashStyle::Both => true,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Error, Display)]
pub(crate) enum CliError {
    #[display("unsupported debug print: {f0}")]
    UnsupportedDebugPrint(String),
    #[display("failed to parse debug print filter")]
    BadObjectsFilter(#[from] ObjectsFilterParseError),
    #[display("failed to parse debug print filter")]
    BadRenderElfFilter(#[from] RenderElfFiltersParseError),
    #[display("debug print enabled multiple times: {f0}")]
    DuplicateDebugPrint(String),
    #[display("flag {f0} is not supported")]
    UnsupportedFlag(String),
    #[display("flag {f0} provided multiple times")]
    DuplicateFlag(String),
    #[display("multiple flags changing the linking mode are passed")]
    MultipleModeChanges,
    #[display("flag {f0} does not accept values")]
    FlagDoesNotAcceptValues(String),
    #[display("missing value for flag {f0}")]
    MissingValueForFlag(String),
    #[display("-z relro is only supported in PIE mode")]
    RelroOnlyForPie,
    #[display("-z now is only supported in PIE mode")]
    NowOnlyForPie,
    #[display("setting the shared object name is only supported when building shared objects")]
    UnsupportedSharedObjectName,
    #[display("sysroot-relative library paths are not supported yet")]
    UnsupportedSysrootRelativeLibraryPath,
    #[display("unsupported hash style {f0}")]
    UnsupportedHashStyle(String),
}
