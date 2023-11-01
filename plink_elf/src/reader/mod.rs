mod cursor;
mod notes;
mod object;
mod program_header;
mod sections;

pub(crate) use self::cursor::Cursor;
pub(crate) use self::object::read_object;

use crate::ids::ElfIds;

#[derive(Debug)]
struct PendingIds;

impl ElfIds for PendingIds {
    type SectionId = PendingSectionId;
    type SymbolId = PendingSymbolId;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct PendingSectionId(u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct PendingSymbolId(PendingSectionId, u32);
