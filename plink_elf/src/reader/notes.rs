use crate::errors::LoadError;
use crate::reader::Cursor;
use crate::{Note, NotesTable, RawBytes};

pub(super) fn read_notes(
    cursor: &mut Cursor<'_>,
    raw_content: &[u8],
) -> Result<NotesTable, LoadError> {
    let mut inner = std::io::Cursor::new(raw_content);
    let mut cursor = cursor.duplicate(&mut inner);

    let mut notes = Vec::new();
    while cursor.current_position()? != raw_content.len() as u64 {
        notes.push(read_note(&mut cursor)?);
    }

    Ok(NotesTable { notes })
}

fn read_note(cursor: &mut Cursor<'_>) -> Result<Note, LoadError> {
    let name_size = cursor.read_u32()?;
    let value_size = cursor.read_u32()?;
    let type_ = cursor.read_u32()?;

    let mut name_bytes = cursor.read_vec(name_size as _)?;
    name_bytes.pop(); // Zero-terminated string
    cursor.align_with_padding(8)?;

    let value_bytes = cursor.read_vec(value_size as _)?;
    cursor.align_with_padding(8)?;

    Ok(Note {
        name: String::from_utf8(name_bytes)?,
        value: RawBytes(value_bytes),
        type_,
    })
}
