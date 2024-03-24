use crate::interner::intern;
use crate::repr::symbols::{Symbol, SymbolValue, Symbols};
use plinky_ar::{ArFile, ArMemberId, ArReadError, ArReader};
use plinky_diagnostics::{Diagnostic, ObjectSpan};
use plinky_elf::errors::LoadError;
use plinky_elf::ids::serial::SerialIds;
use plinky_elf::ElfObject;
use plinky_macros::{Display, Error};
use std::collections::{HashSet, VecDeque};
use std::fs::File;
use std::io::{BufReader, Cursor, Read};
use std::path::{Path, PathBuf};

type ObjectItem = (ObjectSpan, ElfObject<SerialIds>);

pub(super) struct ObjectsReader<'a> {
    remaining_files: &'a [PathBuf],
    current_archive: Option<PendingArchive>,
}

impl<'a> ObjectsReader<'a> {
    pub(super) fn new(paths: &'a [PathBuf]) -> Self {
        Self { remaining_files: paths, current_archive: None }
    }

    pub(super) fn next_object(
        &mut self,
        ids: &mut SerialIds,
        symbols: &Symbols,
    ) -> Result<Option<ObjectItem>, ReadObjectsError> {
        loop {
            if let Some(result) = self.next_from_archive(ids)? {
                return Ok(Some(result));
            }

            if self.remaining_files.is_empty() {
                return Ok(None);
            }
            let path = &self.remaining_files[0];
            self.remaining_files = &self.remaining_files[1..];

            let mut r = BufReader::new(
                File::open(path).map_err(|e| ReadObjectsError::OpenFailed(path.clone(), e))?,
            );
            match FileType::from_magic_number(path, &mut r)? {
                FileType::Elf => {
                    return Ok(Some((
                        ObjectSpan::new_file(path),
                        ElfObject::load(&mut r, ids)
                            .map_err(|e| ReadObjectsError::FileParseFailed(path.clone(), e))?,
                    )))
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

    fn next_from_archive(
        &mut self,
        ids: &mut SerialIds,
    ) -> Result<Option<ObjectItem>, ReadObjectsError> {
        let Some(pending_archive) = &mut self.current_archive else { return Ok(None) };
        match pending_archive.next()? {
            Some(file) => match ElfObject::load(&mut Cursor::new(file.content), ids) {
                Ok(object) => Ok(Some((
                    ObjectSpan::new_archive_member(&pending_archive.path, file.name),
                    object,
                ))),
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
}

impl PendingArchive {
    fn new(
        path: PathBuf,
        reader: BufReader<File>,
        symbols: &Symbols,
    ) -> Result<Option<Self>, ReadObjectsError> {
        let reader =
            ArReader::new(reader).map_err(|e| ReadObjectsError::ExtractFailed(path.clone(), e))?;

        let Some(symbol_table) = reader.symbol_table().cloned() else {
            return Err(ReadObjectsError::NoSymbolTableAtArchiveStart {
                diagnostic: crate::diagnostics::no_symbol_table_at_archive_start::build(&path),
                path,
            });
        };

        let mut pending_members = VecDeque::new();
        let mut pending_members_set = HashSet::new();
        for (symbol_name, member_id) in symbol_table.symbols {
            if let Ok(Symbol { value: SymbolValue::Undefined, .. }) =
                symbols.get_global(intern(&symbol_name))
            {
                // We want to maintain the ordering of the ArMemberId to ensure determinism in the
                // linker output (aka we need to store it in a Vec). The HashSet is used as a quick
                // way to lookup, since it doesn't preserve ordering.
                if pending_members_set.insert(member_id) {
                    pending_members.push_back(member_id);
                }
            }
        }

        Ok(Some(PendingArchive { path, reader, pending_members }))
    }

    fn next(&mut self) -> Result<Option<ArFile>, ReadObjectsError> {
        if let Some(member_id) = self.pending_members.pop_front() {
            Ok(Some(
                self.reader
                    .read_member_by_id(&member_id)
                    .map_err(|e| ReadObjectsError::ExtractFailed(self.path.clone(), e))?,
            ))
        } else {
            Ok(None)
        }
    }
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
}
