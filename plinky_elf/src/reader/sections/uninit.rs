use crate::errors::LoadError;
use crate::reader::sections::SectionReader;
use crate::{ElfSectionContent, ElfUninitializedSection};

pub(super) fn read(reader: &mut SectionReader<'_, '_>) -> Result<ElfSectionContent, LoadError> {
    Ok(ElfSectionContent::Uninitialized(ElfUninitializedSection {
        perms: reader.permissions(),
        len: reader.header.size,
    }))
}
