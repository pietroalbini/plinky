use crate::errors::LoadError;
use crate::reader::sections::SectionReader;
use crate::{ElfSectionContent, ElfUnknownSection};

pub(super) fn read(
    reader: &mut SectionReader<'_, '_>,
    kind: u32,
) -> Result<ElfSectionContent, LoadError> {
    Ok(ElfSectionContent::Unknown(ElfUnknownSection { id: kind, raw: reader.content()? }))
}
