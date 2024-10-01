mod cursor;
mod notes;
mod object;
mod program_header;
mod sections;

pub(crate) use self::cursor::ReadCursor;
pub(crate) use self::object::read_object;
pub use self::cursor::ReadSeek;

use crate::ids::{ElfIds, ReprIdGetters, StringIdGetters};

#[derive(Debug, Clone, Copy)]
pub struct PendingIds;

impl ElfIds for PendingIds {
    type SectionId = PendingSectionId;
    type SymbolId = PendingSymbolId;
    type StringId = PendingStringId;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PendingSectionId(u32);

impl ReprIdGetters for PendingSectionId {
    fn repr_id(&self) -> String {
        format!("{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PendingSymbolId(PendingSectionId, u32);

impl ReprIdGetters for PendingSymbolId {
    fn repr_id(&self) -> String {
        format!("{}#{}", self.0.repr_id(), self.1)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PendingStringId(PendingSectionId, u32);

impl StringIdGetters<PendingIds> for PendingStringId {
    fn section(&self) -> &PendingSectionId {
        &self.0
    }

    fn offset(&self) -> u32 {
        self.1
    }
}
