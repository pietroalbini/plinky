use crate::errors::WriteError;
use crate::writer::cursor::WriteCursor;
use std::collections::BTreeMap;

pub(super) struct Replacements {
    pending: BTreeMap<Replacement, PendingReplacement>,
    known: BTreeMap<Replacement, u64>,
}

impl Replacements {
    pub(super) fn new() -> Self {
        Replacements {
            pending: BTreeMap::new(),
            known: BTreeMap::new(),
        }
    }

    pub(super) fn write(
        &mut self,
        cursor: &mut WriteCursor<'_>,
        replacement: Replacement,
    ) -> Result<(), WriteError> {
        if let Some(value) = self.known.get(&replacement) {
            replacement.write(cursor, *value)
        } else {
            self.pending
                .entry(replacement)
                .or_insert_with(|| PendingReplacement {
                    locations: Vec::new(),
                })
                .locations
                .push(cursor.current_location()?);
            replacement.write(cursor, 0)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Replacement {
    ProgramHeaderOffset,
    SectionHeaderOffset,
    HeaderSize,
    ProgramHeaderEntrySize,
    SectionHeaderEntrySize,
}

impl Replacement {
    fn write(&self, cursor: &mut WriteCursor<'_>, value: u64) -> Result<(), WriteError> {
        match self {
            Replacement::ProgramHeaderOffset => cursor.write_usize(value),
            Replacement::SectionHeaderOffset => cursor.write_usize(value),
            Replacement::HeaderSize => cursor.write_u16(value as _),
            Replacement::ProgramHeaderEntrySize => cursor.write_u16(value as _),
            Replacement::SectionHeaderEntrySize => cursor.write_u16(value as _),
        }
    }
}

struct PendingReplacement {
    locations: Vec<u64>,
}
