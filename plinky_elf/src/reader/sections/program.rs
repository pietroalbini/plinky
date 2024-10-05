use crate::errors::LoadError;
use crate::reader::sections::SectionReader;
use crate::{ElfProgramSection, ElfSectionContent};

pub(super) fn read(reader: &mut SectionReader<'_, '_>) -> Result<ElfSectionContent, LoadError> {
    Ok(ElfSectionContent::Program(ElfProgramSection {
        perms: reader.permissions(),
        deduplication: reader.deduplication_flag()?,
        raw: reader.content()?,
    }))
}
