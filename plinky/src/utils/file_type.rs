use plinky_macros::{Display, Error};
use std::fs::File;
use std::io::{BufReader, Read as _};
use std::path::{Path, PathBuf};

pub(crate) enum FileType {
    Elf,
    Ar,
}

impl FileType {
    pub(crate) fn from_magic_number(
        path: &Path,
        reader: &mut BufReader<File>,
    ) -> Result<Self, FileTypeError> {
        let io_err = |e| FileTypeError::ReadFailed(path.into(), e);

        let mut magic = [0; 8];
        reader.read_exact(&mut magic).map_err(io_err)?;
        reader.seek_relative(-(magic.len() as i64)).map_err(io_err)?;

        match &magic {
            [0x7F, b'E', b'L', b'F', ..] => Ok(FileType::Elf),
            b"!<arch>\n" => Ok(FileType::Ar),
            _ => Err(FileTypeError::Unsupported(path.into())),
        }
    }
}

#[derive(Debug, Display, Error)]
pub(crate) enum FileTypeError {
    #[display("unsupported file type for {f0:?}")]
    Unsupported(PathBuf),
    #[display("failed to read the magic number to detect the file type of {f0:?}")]
    ReadFailed(PathBuf, #[source] std::io::Error),
}
