use crate::passes::load_inputs::ObjectLocation;
use plink_ar::{ArReadError, ArReader};
use plink_elf::errors::LoadError;
use plink_elf::ids::serial::SerialIds;
use plink_elf::ElfObject;
use plink_macros::{Display, Error};
use std::fs::File;
use std::io::{BufReader, Cursor, Read};
use std::path::{Path, PathBuf};

type ObjectItem = (ObjectLocation, ElfObject<SerialIds>);
type IterItem = Result<ObjectItem, ReadObjectsError>;

pub(super) fn objects_iter<'a>(
    paths: &'a [PathBuf],
    ids: &'a mut SerialIds,
) -> impl Iterator<Item = IterItem> + 'a {
    ObjectsReader { remaining_files: paths, current_archive: None, ids }
}

struct ObjectsReader<'a> {
    remaining_files: &'a [PathBuf],
    current_archive: Option<(PathBuf, ArReader<BufReader<File>>)>,
    ids: &'a mut SerialIds,
}

impl ObjectsReader<'_> {
    fn next_object(&mut self) -> Result<Option<ObjectItem>, ReadObjectsError> {
        loop {
            if let Some(result) = self.next_from_archive()? {
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
                        ObjectLocation::File(path.clone()),
                        ElfObject::load(&mut r, self.ids)
                            .map_err(|e| ReadObjectsError::FileParseFailed(path.clone(), e))?,
                    )))
                }
                FileType::Ar => {
                    self.current_archive = Some((
                        path.clone(),
                        ArReader::new(r)
                            .map_err(|e| ReadObjectsError::ExtractFailed(path.clone(), e))?,
                    ));
                    continue;
                }
            }
        }
    }

    fn next_from_archive(&mut self) -> Result<Option<ObjectItem>, ReadObjectsError> {
        let Some((path, archive)) = &mut self.current_archive else { return Ok(None) };
        match archive.next() {
            Some(file) => {
                let file = file.map_err(|e| ReadObjectsError::ExtractFailed(path.clone(), e))?;
                match ElfObject::load(&mut Cursor::new(file.content), self.ids) {
                    Ok(object) => Ok(Some((
                        ObjectLocation::Archive { archive: path.clone(), member: file.name },
                        object,
                    ))),
                    Err(err) => {
                        Err(ReadObjectsError::ArchiveFileParseFailed(file.name, path.clone(), err))
                    }
                }
            }
            None => {
                self.current_archive = None;
                Ok(None)
            }
        }
    }
}

impl Iterator for ObjectsReader<'_> {
    type Item = IterItem;

    fn next(&mut self) -> Option<Self::Item> {
        self.next_object().transpose()
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
}
