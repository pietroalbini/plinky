use crate::errors::LoadError;
use crate::reader::Cursor;
use crate::{ElfClass, ElfSegment, ElfSegmentType};

pub(super) fn read_program_header(cursor: &mut Cursor<'_>) -> Result<ElfSegment, LoadError> {
    // The position of the `flags` field changes depending on whether it's a 32-bit or 64-bit
    // ELF binary.
    let mut flags = 0;

    let type_ = cursor.read_u32()?;
    if let Some(ElfClass::Elf64) = &cursor.class {
        flags = cursor.read_u32()?;
    }
    let file_offset = cursor.read_usize()?;
    let virtual_address = cursor.read_usize()?;
    let _reserved = cursor.read_usize()?;
    let file_size = cursor.read_usize()?;
    let memory_size = cursor.read_usize()?;
    if let Some(ElfClass::Elf32) = &cursor.class {
        flags = cursor.read_u32()?;
    }
    let align = cursor.read_usize()?;

    Ok(ElfSegment {
        type_: match type_ {
            0 => ElfSegmentType::Null,
            1 => ElfSegmentType::Load,
            2 => ElfSegmentType::Dynamic,
            3 => ElfSegmentType::Interpreter,
            4 => ElfSegmentType::Note,
            6 => ElfSegmentType::ProgramHeaderTable,
            other => ElfSegmentType::Unknown(other),
        },
        readable: flags & 0x4 > 0,
        writeable: flags & 0x2 > 0,
        executable: flags & 0x1 > 0,
        file_offset,
        virtual_address,
        file_size,
        memory_size,
        align,
    })
}
