use crate::errors::LoadError;
use crate::reader::sections::reader::{SectionMetadata, SectionReader};
use crate::ElfProgramSection;

pub(super) fn read(
    reader: &mut SectionReader<'_, '_>,
    meta: &dyn SectionMetadata,
) -> Result<ElfProgramSection, LoadError> {
    Ok(ElfProgramSection {
        perms: meta.permissions(),
        deduplication: meta.deduplication_flag()?,
        raw: reader.content()?,
    })
}
