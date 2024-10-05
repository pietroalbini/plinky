use crate::errors::LoadError;
use crate::ids::{ElfSectionId, ElfSymbolId};
use crate::raw::{RawGroupFlags, RawSectionHeader};
use crate::reader::ReadCursor;
use crate::{ElfGroup, ElfSectionContent};

pub(super) fn read(
    header: &RawSectionHeader,
    cursor: &mut ReadCursor<'_>,
    raw_content: &[u8],
) -> Result<ElfSectionContent, LoadError> {
    let mut inner = std::io::Cursor::new(raw_content);
    let mut cursor = cursor.duplicate(&mut inner);

    let symbol_table = ElfSectionId { index: header.link };
    let signature = ElfSymbolId { section: symbol_table, index: header.info };

    let flags: RawGroupFlags = cursor.read_raw()?;

    let mut sections = Vec::new();
    while cursor.current_position()? < raw_content.len() as u64 {
        sections.push(ElfSectionId { index: cursor.read_raw::<u32>()? });
    }

    Ok(ElfSectionContent::Group(ElfGroup {
        symbol_table,
        signature,
        sections,
        comdat: flags.comdat,
    }))
}
