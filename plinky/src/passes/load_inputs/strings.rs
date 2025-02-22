use plinky_elf::ElfStringTable;
use plinky_elf::ids::{ElfSectionId, ElfStringId};
use plinky_macros::{Display, Error};
use std::collections::BTreeMap;

#[derive(Debug)]
pub(super) struct Strings {
    tables: BTreeMap<ElfSectionId, ElfStringTable>,
}

impl Strings {
    pub(super) fn new() -> Self {
        Self { tables: BTreeMap::new() }
    }

    pub(super) fn load_table(&mut self, section_id: ElfSectionId, table: ElfStringTable) {
        self.tables.insert(section_id, table);
    }

    pub(super) fn get(&self, id: ElfStringId) -> Result<&str, MissingStringError> {
        self.tables
            .get(&id.section)
            .and_then(|table| table.get(id.offset))
            .ok_or(MissingStringError(id))
    }
}

#[derive(Debug, Error, Display)]
#[display("missing string {f0:?}")]
pub(crate) struct MissingStringError(ElfStringId);
