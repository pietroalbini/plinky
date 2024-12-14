mod lexer;
mod parser;
#[cfg(test)]
mod tests;

pub(crate) use crate::cli::parser::parse;
use crate::debug_print::filters::{ObjectsFilter, ObjectsFilterParseError};
use plinky_elf::render_elf::{RenderElfFilters, RenderElfFiltersParseError};
use plinky_macros::{Display, Error};
use std::collections::BTreeSet;
use std::path::PathBuf;

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct CliOptions {
    pub(crate) inputs: Vec<CliInput>,
    pub(crate) output: PathBuf,
    pub(crate) entry: Option<String>,
    pub(crate) gc_sections: bool,
    pub(crate) debug_print: BTreeSet<DebugPrint>,
    pub(crate) executable_stack: bool,
    pub(crate) read_only_got: bool,
    pub(crate) read_only_got_plt: bool,
    pub(crate) dynamic_linker: DynamicLinker,
    pub(crate) search_paths: Vec<PathBuf>,
    pub(crate) shared_object_name: Option<String>,
    pub(crate) mode: Mode,
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
pub(crate) enum CliInput {
    Path(PathBuf),
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
}
