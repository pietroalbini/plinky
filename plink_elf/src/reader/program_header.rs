use crate::errors::LoadError;
use crate::reader::Cursor;
use crate::{Class, RawBytes, Segment, SegmentContent};

pub(super) fn read_program_header(cursor: &mut Cursor<'_>) -> Result<Segment, LoadError> {
    // The position of the `flags` field changes depending on whether it's a 32-bit or 64-bit
    // ELF binary.
    let mut _flags = 0;

    let type_ = cursor.read_u32()?;
    if let Some(Class::Elf64) = &cursor.class {
        _flags = cursor.read_u32()?;
    }
    let offset = cursor.read_usize()?;
    let _virtual_address = cursor.read_usize()?;
    let _reserved = cursor.read_usize()?;
    let file_size = cursor.read_usize()?;
    let _memory_size = cursor.read_usize()?;
    if let Some(Class::Elf32) = &cursor.class {
        _flags = cursor.read_u32()?;
    }
    let _align = cursor.read_usize()?;
    cursor.seek_to(offset)?;
    let contents = cursor.read_vec(file_size)?;

    Ok(Segment {
        content: SegmentContent::Unknown {
            id: type_,
            raw: RawBytes(contents),
        },
    })
}
