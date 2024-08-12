use crate::errors::LoadError;
use crate::raw::RawProgramHeader;
use crate::reader::ReadCursor;
use crate::{ElfPermissions, ElfSegment, ElfSegmentType};

pub(super) fn read_program_header(cursor: &mut ReadCursor<'_>) -> Result<ElfSegment, LoadError> {
    let header: RawProgramHeader = cursor.read_raw()?;

    Ok(ElfSegment {
        type_: match header.type_ {
            0 => ElfSegmentType::Null,
            1 => ElfSegmentType::Load,
            2 => ElfSegmentType::Dynamic,
            3 => ElfSegmentType::Interpreter,
            4 => ElfSegmentType::Note,
            6 => ElfSegmentType::ProgramHeaderTable,
            0x6474e551 => ElfSegmentType::GnuStack,
            0x6474e552 => ElfSegmentType::GnuRelRO,
            other => ElfSegmentType::Unknown(other),
        },
        perms: ElfPermissions {
            read: header.flags.read,
            write: header.flags.write,
            execute: header.flags.execute,
        },

        file_offset: header.file_offset,
        file_size: header.file_size,
        virtual_address: header.virtual_address,
        memory_size: header.memory_size,
        align: header.align,
    })
}
