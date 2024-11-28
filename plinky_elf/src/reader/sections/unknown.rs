use crate::ElfUnknownSection;
use crate::errors::LoadError;
use crate::reader::sections::SectionReader;

pub(super) fn read(
    reader: &mut SectionReader<'_, '_>,
    kind: u32,
) -> Result<ElfUnknownSection, LoadError> {
    Ok(ElfUnknownSection { id: kind, raw: reader.content()? })
}
