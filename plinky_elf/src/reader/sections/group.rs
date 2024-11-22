use crate::errors::LoadError;
use crate::ids::{ElfSectionId, ElfSymbolId};
use crate::raw::RawGroupFlags;
use crate::reader::sections::reader::{SectionMetadata, SectionReader};
use crate::ElfGroup;

pub(super) fn read(
    reader: &mut SectionReader<'_, '_>,
    meta: &dyn SectionMetadata,
) -> Result<ElfGroup, LoadError> {
    let mut cursor = reader.content_cursor()?;

    let symbol_table = meta.section_link();
    let signature = ElfSymbolId { section: symbol_table, index: meta.info_field() };

    let flags: RawGroupFlags = cursor.read_raw()?;

    let mut sections = Vec::new();
    while cursor.current_position()? < reader.content_len as u64 {
        sections.push(ElfSectionId { index: cursor.read_raw::<u32>()? });
    }

    Ok(ElfGroup { symbol_table, signature, sections, comdat: flags.comdat })
}
