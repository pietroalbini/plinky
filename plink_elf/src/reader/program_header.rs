use crate::errors::LoadError;
use crate::reader::Cursor;
use crate::{ElfClass, ElfSegment, ElfSegmentContent, RawBytes};

pub(super) fn read_program_header(cursor: &mut Cursor<'_>) -> Result<ElfSegment, LoadError> {
    // The position of the `flags` field changes depending on whether it's a 32-bit or 64-bit
    // ELF binary.
    let mut _flags = 0;

    let type_ = cursor.read_u32()?;
    if let Some(ElfClass::Elf64) = &cursor.class {
        _flags = cursor.read_u32()?;
    }
    let offset = cursor.read_usize()?;
    let _virtual_address = cursor.read_usize()?;
    let _reserved = cursor.read_usize()?;
    let file_size = cursor.read_usize()?;
    let _memory_size = cursor.read_usize()?;
    if let Some(ElfClass::Elf32) = &cursor.class {
        _flags = cursor.read_u32()?;
    }
    let _align = cursor.read_usize()?;
    cursor.seek_to(offset)?;
    let contents = cursor.read_vec(file_size)?;

    Ok(ElfSegment {
        content: ElfSegmentContent::Unknown {
            id: type_,
            raw: RawBytes(contents),
        },
    })
}
