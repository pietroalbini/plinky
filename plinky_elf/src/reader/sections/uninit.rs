use crate::errors::LoadError;
use crate::reader::sections::{SectionMetadata, SectionReader};
use crate::{ElfSectionContent, ElfUninitializedSection};

pub(super) fn read(
    reader: &mut SectionReader<'_, '_>,
    meta: &dyn SectionMetadata,
) -> Result<ElfSectionContent, LoadError> {
    Ok(ElfSectionContent::Uninitialized(ElfUninitializedSection {
        perms: meta.permissions(),
        len: reader.content_len,
    }))
}
