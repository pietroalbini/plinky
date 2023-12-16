use plink_elf::ids::serial::{SectionId, StringId};
use plink_elf::ids::StringIdGetters;
use plink_elf::ElfStringTable;
use plink_macros::{Display, Error};
use std::collections::BTreeMap;

#[derive(Debug)]
pub(crate) struct Strings {
    tables: BTreeMap<SectionId, ElfStringTable>,
}

impl Strings {
    pub(crate) fn new() -> Self {
        Self { tables: BTreeMap::new() }
    }

    pub(crate) fn load_table(&mut self, section_id: SectionId, table: ElfStringTable) {
        self.tables.insert(section_id, table);
    }

    pub(crate) fn get(&self, id: StringId) -> Result<&str, MissingStringError> {
        self.tables
            .get(id.section())
            .and_then(|table| table.get(id.offset()))
            .ok_or(MissingStringError(id))
    }
}

#[derive(Debug, Error, Display)]
#[display("missing string {f0:?}")]
pub(crate) struct MissingStringError(StringId);
