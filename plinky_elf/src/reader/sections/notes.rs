use crate::errors::LoadError;
use crate::raw::RawNoteHeader;
use crate::reader::ReadCursor;
use crate::{ElfNote, ElfNotesTable, ElfSectionContent, ElfUnknownNote};

pub(super) fn read(
    cursor: &mut ReadCursor<'_>,
    raw_content: &[u8],
) -> Result<ElfSectionContent, LoadError> {
    let mut inner = std::io::Cursor::new(raw_content);
    let mut cursor = cursor.duplicate(&mut inner);

    let mut notes = Vec::new();
    while cursor.current_position()? != raw_content.len() as u64 {
        notes.push(read_note(&mut cursor)?);
    }

    Ok(ElfSectionContent::Note(ElfNotesTable { notes }))
}

fn read_note(cursor: &mut ReadCursor<'_>) -> Result<ElfNote, LoadError> {
    let header: RawNoteHeader = cursor.read_raw()?;

    let mut name_bytes = cursor.read_vec(header.name_size as _)?;
    name_bytes.pop(); // Zero-terminated string
    cursor.align_with_padding(8)?;

    let value = cursor.read_vec(header.value_size as _)?;
    cursor.align_with_padding(8)?;

    Ok(ElfNote::Unknown(ElfUnknownNote {
        name: String::from_utf8(name_bytes)?,
        value,
        type_: header.type_,
    }))
}
