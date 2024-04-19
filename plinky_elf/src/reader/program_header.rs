use crate::errors::LoadError;
use crate::raw::RawProgramHeader;
use crate::reader::{PendingIds, PendingSectionId, ReadCursor};
use crate::{
    ElfPermissions, ElfSegment, ElfSegmentContent, ElfSegmentType, ElfUnknownSegmentContent,
};
use std::collections::BTreeMap;

pub(super) type SegmentContentMapping = BTreeMap<(u64, u64), PendingSectionId>;

pub(super) fn read_program_header(
    cursor: &mut ReadCursor<'_>,
    content_map: &SegmentContentMapping,
) -> Result<ElfSegment<PendingIds>, LoadError> {
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
            other => ElfSegmentType::Unknown(other),
        },
        perms: ElfPermissions {
            read: header.flags.read,
            write: header.flags.write,
            execute: header.flags.execute,
        },

        content: if header.file_size == 0
            && header.file_offset == 0
            && header.virtual_address == 0
            && header.memory_size == 0
        {
            ElfSegmentContent::Empty
        } else if let Some(id) = content_map.get(&(header.file_offset, header.file_size)) {
            ElfSegmentContent::Sections(vec![id.clone()])
        } else {
            ElfSegmentContent::Unknown(ElfUnknownSegmentContent {
                file_offset: header.file_offset,
                virtual_address: header.virtual_address,
                file_size: header.file_size,
                memory_size: header.memory_size,
            })
        },

        align: header.align,
    })
}
