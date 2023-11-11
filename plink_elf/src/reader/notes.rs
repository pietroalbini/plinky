use crate::errors::LoadError;
use crate::reader::ReadCursor;
use crate::{ElfNote, ElfNotesTable, RawBytes};

pub(super) fn read_notes(
    cursor: &mut ReadCursor<'_>,
    raw_content: &[u8],
) -> Result<ElfNotesTable, LoadError> {
    let mut inner = std::io::Cursor::new(raw_content);
    let mut cursor = cursor.duplicate(&mut inner);

    let mut notes = Vec::new();
    while cursor.current_position()? != raw_content.len() as u64 {
        notes.push(read_note(&mut cursor)?);
    }

    Ok(ElfNotesTable { notes })
}

fn read_note(cursor: &mut ReadCursor<'_>) -> Result<ElfNote, LoadError> {
    let name_size = cursor.read_u32()?;
    let value_size = cursor.read_u32()?;
    let type_ = cursor.read_u32()?;

    let mut name_bytes = cursor.read_vec(name_size as _)?;
    name_bytes.pop(); // Zero-terminated string
    cursor.align_with_padding(8)?;

    let value_bytes = cursor.read_vec(value_size as _)?;
    cursor.align_with_padding(8)?;

    Ok(ElfNote {
        name: String::from_utf8(name_bytes)?,
        value: RawBytes(value_bytes),
        type_,
    })
}
