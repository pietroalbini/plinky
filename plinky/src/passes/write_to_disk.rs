use crate::passes::build_elf::ids::BuiltElfIds;
use plinky_elf::errors::WriteError;
use plinky_elf::writer::Writer;
use plinky_elf::ElfObject;
use plinky_macros::Error;
use std::fs::{File, Permissions};
use std::io::BufWriter;
use std::os::unix::prelude::PermissionsExt;
use std::path::{Path, PathBuf};

pub(crate) fn run(object: ElfObject<BuiltElfIds>, dest: &Path) -> Result<(), WriteToDiskError> {
    let mut file = BufWriter::new(
        File::create(dest).map_err(|e| WriteToDiskError::FileCreation(dest.into(), e))?,
    );

    Writer::new(&mut file, &object)
        .and_then(|w| w.write())
        .map_err(|e| WriteToDiskError::WriteFailed(dest.into(), e))?;

    std::fs::set_permissions(dest, Permissions::from_mode(0o755))
        .map_err(|e| WriteToDiskError::PermissionSetFailed(dest.into(), e))?;

    Ok(())
}

#[derive(Debug, Error)]
pub(crate) enum WriteToDiskError {
    FileCreation(PathBuf, #[source] std::io::Error),
    WriteFailed(PathBuf, #[source] WriteError<BuiltElfIds>),
    PermissionSetFailed(PathBuf, #[source] std::io::Error),
}

impl std::fmt::Display for WriteToDiskError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WriteToDiskError::FileCreation(path, _) => {
                write!(f, "failed to create output file at {}", path.display())
            }
            WriteToDiskError::WriteFailed(path, _) => {
                write!(f, "failed to serialize output to {}", path.display())
            }
            WriteToDiskError::PermissionSetFailed(path, _) => {
                write!(f, "failed to make {} executable", path.display())
            }
        }
    }
}
