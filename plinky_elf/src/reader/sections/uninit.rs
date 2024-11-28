use crate::ElfUninitializedSection;
use crate::errors::LoadError;
use crate::reader::sections::reader::{SectionMetadata, SectionReader};

pub(super) fn read(
    reader: &mut SectionReader<'_, '_>,
    meta: &dyn SectionMetadata,
) -> Result<ElfUninitializedSection, LoadError> {
    Ok(ElfUninitializedSection { perms: meta.permissions(), len: reader.content_len })
}
