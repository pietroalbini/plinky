use crate::errors::LoadError;
use crate::raw::RawProgramHeader;
use crate::reader::{PendingIds, ReadCursor};
use crate::{
    ElfPermissions, ElfSegment, ElfSegmentContent, ElfSegmentType, ElfUnknownSegmentContent,
};
use std::collections::BTreeMap;

pub(super) type SegmentContentMapping = BTreeMap<(u64, u64), ElfSegmentContent<PendingIds>>;

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
            other => ElfSegmentType::Unknown(other),
        },
        perms: ElfPermissions {
            read: header.flags.read,
            write: header.flags.write,
            execute: header.flags.execute,
        },
        content: vec![content_map
            .get(&(header.file_offset, header.file_size))
            .cloned()
            .unwrap_or(ElfSegmentContent::Unknown(ElfUnknownSegmentContent {
                file_offset: header.file_offset,
                virtual_address: header.virtual_address,
                file_size: header.file_size,
                memory_size: header.memory_size,
            }))],
        align: header.align,
    })
}
