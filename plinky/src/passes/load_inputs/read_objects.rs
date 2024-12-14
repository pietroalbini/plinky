use crate::cli::{CliInput, CliInputValue};
use crate::interner::intern;
use crate::repr::symbols::{SymbolValue, Symbols};
use plinky_ar::{ArFile, ArMemberId, ArReadError, ArReader};
use plinky_diagnostics::{Diagnostic, ObjectSpan};
use plinky_elf::{ElfReader, LoadError};
use plinky_macros::{Display, Error};
use std::collections::{HashSet, VecDeque};
use std::fs::File;
use std::io::{BufReader, Cursor, Read};
use std::path::{Path, PathBuf};

pub(super) struct ObjectsReader<'a> {
    search_paths: &'a [PathBuf],
    remaining_inputs: &'a [CliInput],
    current_archive: Option<PendingArchive>,
}

impl<'a> ObjectsReader<'a> {
    pub(super) fn new(search_paths: &'a [PathBuf], inputs: &'a [CliInput]) -> Self {
        Self { remaining_inputs: inputs, current_archive: None, search_paths }
    }

    pub(super) fn next_object(
        &mut self,
        symbols: &Symbols,
    ) -> Result<Option<NextObject>, ReadObjectsError> {
        loop {
            if let Some(result) = self.next_from_archive(symbols)? {
                return Ok(Some(result));
            }

            if self.remaining_inputs.is_empty() {
                return Ok(None);
            }
            let input = &self.remaining_inputs[0];
            self.remaining_inputs = &self.remaining_inputs[1..];

            let (path, library_name) = self.resolve_input(input)?;

            let mut r = BufReader::new(
                File::open(&path).map_err(|e| ReadObjectsError::OpenFailed(path.clone(), e))?,
            );
            match FileType::from_magic_number(&path, &mut r)? {
                FileType::Elf => {
                    return Ok(Some(NextObject {
                        source: ObjectSpan::new_file(&path),
                        library_name,
                        reader: ElfReader::new_owned(Box::new(r))
                            .map_err(|e| ReadObjectsError::FileParseFailed(path.clone(), e))?,
                    }));
                }
                FileType::Ar => {
                    if let Some(archive) = PendingArchive::new(path.clone(), r, symbols)? {
                        self.current_archive = Some(archive);
                    }
                    continue;
                }
            }
        }
    }

    fn resolve_input(&self, input: &CliInput) -> Result<(PathBuf, LibraryName), ReadObjectsError> {
        let mut names = Vec::new();
        match &input.value {
            CliInputValue::Path(p) => {
                return Ok((p.clone(), LibraryName::Known(path_to_string(&p)?)))
            }
            CliInputValue::Library(name) => {
                if input.search_shared_objects {
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

        for search_path in self.search_paths {
            for name in &names {
                let path = search_path.join(name);
                match std::fs::metadata(&path) {
                    Ok(_) => {
                        return Ok((
                            path,
                            // When a library is linked with -l, the library path that will be
                            // included in DT_NEEDED won't be the real path of the library, but
                            // just the file name (assuming the library doesn't have DT_SONAME).
                            LibraryName::Known(name.into()),
                        ));
                    }
                    Err(err) if err.kind() == std::io::ErrorKind::NotFound => continue,
                    Err(err) => {
                        return Err(ReadObjectsError::OpenLibraryCandidate {
                            library: library_for_error(),
                            path,
                            err,
                        });
                    }
                }
            }
        }

        Err(ReadObjectsError::MissingLibrary(library_for_error()))
    }

    fn next_from_archive(
        &mut self,
        symbols: &Symbols,
    ) -> Result<Option<NextObject>, ReadObjectsError> {
        let Some(pending_archive) = &mut self.current_archive else { return Ok(None) };
        match pending_archive.next(symbols)? {
            Some(file) => match ElfReader::new_owned(Box::new(Cursor::new(file.content))) {
                Ok(reader) => Ok(Some(NextObject {
                    source: ObjectSpan::new_archive_member(&pending_archive.path, &file.name),
                    library_name: LibraryName::InsideArchive,
                    reader,
                })),
                Err(err) => Err(ReadObjectsError::ArchiveFileParseFailed(
                    file.name,
                    pending_archive.path.clone(),
                    err,
                )),
            },
            None => {
                self.current_archive = None;
                Ok(None)
            }
        }
    }
}

struct PendingArchive {
    path: PathBuf,
    reader: ArReader<BufReader<File>>,
    pending_members: VecDeque<ArMemberId>,
    loaded_members: HashSet<ArMemberId>,
}

impl PendingArchive {
    fn new(
        path: PathBuf,
        reader: BufReader<File>,
        symbols: &Symbols,
    ) -> Result<Option<Self>, ReadObjectsError> {
        let mut this = PendingArchive {
            reader: ArReader::new(reader)
                .map_err(|e| ReadObjectsError::ExtractFailed(path.clone(), e))?,
            path,
            pending_members: VecDeque::new(),
            loaded_members: HashSet::new(),
        };

        // Only return a new instance of PendingArchive if there are actually objects to load.
        this.calculate_pending(symbols)?;
        Ok(Some(this).filter(|this| !this.pending_members.is_empty()))
    }

    fn calculate_pending(&mut self, symbols: &Symbols) -> Result<(), ReadObjectsError> {
        let Some(symbol_table) = self.reader.symbol_table() else {
            return Err(ReadObjectsError::NoSymbolTableAtArchiveStart {
                diagnostic: crate::diagnostics::no_symbol_table_at_archive_start::build(&self.path),
                path: self.path.clone(),
            });
        };

        for (symbol_name, member_id) in &symbol_table.symbols {
            if let Ok(symbol) = symbols.get_global(intern(symbol_name)) {
                if let SymbolValue::Undefined = symbol.value() {
                    // We want to maintain the ordering of the ArMemberId to ensure determinism in the
                    // linker output (aka we need to store it in a Vec). The HashSet is used as a quick
                    // way to lookup, since it doesn't preserve ordering.
                    //
                    // This also prevents loading the same object file multiple times when scanning
                    // the archive again for new required symbols.
                    if self.loaded_members.insert(*member_id) {
                        self.pending_members.push_back(*member_id);
                    }
                }
            }
        }

        Ok(())
    }

    fn next(&mut self, symbols: &Symbols) -> Result<Option<ArFile>, ReadObjectsError> {
        loop {
            if let Some(member_id) = self.pending_members.pop_front() {
                return Ok(Some(
                    self.reader
                        .read_member_by_id(&member_id)
                        .map_err(|e| ReadObjectsError::ExtractFailed(self.path.clone(), e))?,
                ));
            }

            // After loading the archive, new undefined symbols that can be satisfied by the
            // archive can appear. This can happen for example if an object file later in the
            // archive depends on object files earlier in the archive. We thus continue trying to
            // load from the archive until there are no more pending files to load.
            self.calculate_pending(symbols)?;
            if self.pending_members.is_empty() {
                return Ok(None);
            }
        }
    }
}

fn path_to_string(path: &Path) -> Result<String, ReadObjectsError> {
    Ok(path
        .to_str()
        .ok_or_else(|| ReadObjectsError::NonUtf8FileName { lossy: path.to_string_lossy().into() })?
        .to_string())
}

enum FileType {
    Elf,
    Ar,
}

impl FileType {
    fn from_magic_number(
        path: &Path,
        reader: &mut BufReader<File>,
    ) -> Result<Self, ReadObjectsError> {
        let io_err = |e| ReadObjectsError::MagicNumberReadFailed(path.into(), e);

        let mut magic = [0; 8];
        reader.read_exact(&mut magic).map_err(io_err)?;
        reader.seek_relative(-(magic.len() as i64)).map_err(io_err)?;

        match &magic {
            [0x7F, b'E', b'L', b'F', ..] => Ok(FileType::Elf),
            b"!<arch>\n" => Ok(FileType::Ar),
            _ => Err(ReadObjectsError::UnsupportedFileType),
        }
    }
}

pub(super) struct NextObject {
    pub(super) reader: ElfReader<'static>,
    pub(super) library_name: LibraryName,
    pub(super) source: ObjectSpan,
}

pub(super) enum LibraryName {
    Known(String),
    InsideArchive,
}

#[derive(Debug, Error, Display)]
pub(crate) enum ReadObjectsError {
    #[display("failed to open {f0:?}")]
    OpenFailed(PathBuf, #[source] std::io::Error),
    #[display("failed to read the magic number to detect the file type of {f0:?}")]
    MagicNumberReadFailed(PathBuf, #[source] std::io::Error),
    #[display("failed to extract the next file from the archive {f0:?}")]
    ExtractFailed(PathBuf, #[source] ArReadError),
    #[display("failed to parse archive member {f0} of {f1:?}")]
    ArchiveFileParseFailed(String, PathBuf, #[source] LoadError),
    #[display("failed to parse {f0:?}")]
    FileParseFailed(PathBuf, #[source] LoadError),
    #[display("unsupported file type")]
    UnsupportedFileType,
    #[display("the first member of the archive {path:?} is not a symbol table")]
    NoSymbolTableAtArchiveStart {
        path: PathBuf,
        #[diagnostic]
        diagnostic: Diagnostic,
    },
    #[display("file name is not UTF-8: {lossy}")]
    NonUtf8FileName { lossy: String },
    #[display("failed to open path while searching for library {library}: {path:?}")]
    OpenLibraryCandidate {
        library: String,
        path: PathBuf,
        #[source]
        err: std::io::Error,
    },
    #[display("missing library {f0}")]
    MissingLibrary(String),
}
