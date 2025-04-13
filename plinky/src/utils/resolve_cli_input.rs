use crate::cli::{CliInput, CliInputValue};
use plinky_macros::{Display, Error};
use std::path::{Path, PathBuf};

pub(crate) fn resolve_cli_input(
    search_path: &[PathBuf],
    input: &CliInput,
) -> Result<ResolvedInput, ResolveCliInputError> {
    let mut names = Vec::new();
    match &input.value {
        CliInputValue::Path(path) => {
            return Ok(ResolvedInput { path: path.clone(), library_name: path_to_string(&path)? });
        }
        CliInputValue::Library(name) => {
            if input.options.search_shared_objects {
                names.push(format!("lib{name}.so"));
            }
            names.push(format!("lib{name}.a"));
        }
        CliInputValue::LibraryVerbatim(verbatim) => names.push(verbatim.clone()),
    }

    // This is a closure to only run it lazily.
    let library_for_error = || match &input.value {
        CliInputValue::Path(_) => unreachable!(),
        CliInputValue::Library(name) => name.clone(),
        CliInputValue::LibraryVerbatim(verbatim) => format!(":{verbatim}"),
    };

    for search_path in search_path {
        for name in &names {
            let path = search_path.join(name);
            match std::fs::metadata(&path) {
                Ok(_) => {
                    return Ok(ResolvedInput {
                        path,
                        // When a library is linked with -l, the library path that will be
                        // included in DT_NEEDED won't be the real path of the library, but
                        // just the file name (assuming the library doesn't have DT_SONAME).
                        library_name: name.into(),
                    });
                }
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => continue,
                Err(err) => {
                    return Err(ResolveCliInputError::OpenLibraryCandidate {
                        library: library_for_error(),
                        path,
                        err,
                    });
                }
            }
        }
    }

    Err(ResolveCliInputError::MissingLibrary(library_for_error()))
}

fn path_to_string(path: &Path) -> Result<String, ResolveCliInputError> {
    Ok(path
        .to_str()
        .ok_or_else(|| ResolveCliInputError::NonUtf8FileName {
            lossy: path.to_string_lossy().into(),
        })?
        .to_string())
}

#[derive(Debug)]
pub(crate) struct ResolvedInput {
    pub(crate) path: PathBuf,
    pub(crate) library_name: String,
}

#[derive(Debug, Error, Display)]
pub(crate) enum ResolveCliInputError {
    #[display("missing library {f0}")]
    MissingLibrary(String),
    #[display("file name is not UTF-8: {lossy}")]
    NonUtf8FileName { lossy: String },
    #[display("failed to open path while searching for library {library}: {path:?}")]
    OpenLibraryCandidate {
        library: String,
        path: PathBuf,
        #[source]
        err: std::io::Error,
    },
}
