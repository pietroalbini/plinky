use crate::errors::LoadError;
use crate::reader::sections::{SectionMetadata, SectionReader};
use crate::{ElfProgramSection, ElfSectionContent};

pub(super) fn read(
    reader: &mut SectionReader<'_, '_>,
    meta: &dyn SectionMetadata,
) -> Result<ElfSectionContent, LoadError> {
    Ok(ElfSectionContent::Program(ElfProgramSection {
        perms: meta.permissions(),
        deduplication: meta.deduplication_flag()?,
        raw: reader.content()?,
    }))
}
