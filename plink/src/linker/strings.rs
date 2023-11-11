use plink_elf::ids::serial::{SectionId, StringId};
use plink_elf::ids::StringIdGetters;
use plink_elf::ElfStringTable;
use plink_macros::Error;
use std::collections::BTreeMap;

#[derive(Debug)]
pub(super) struct Strings {
    tables: BTreeMap<SectionId, ElfStringTable>,
}

impl Strings {
    pub(super) fn new() -> Self {
        Self {
            tables: BTreeMap::new(),
        }
    }

    pub(super) fn load_table(&mut self, section_id: SectionId, table: ElfStringTable) {
        self.tables.insert(section_id, table);
    }

    pub(super) fn get(&self, id: StringId) -> Result<&str, MissingStringError> {
        self.tables
            .get(id.section())
            .and_then(|table| table.get(id.offset()))
            .ok_or(MissingStringError(id))
    }
}

#[derive(Debug, Error)]
pub(crate) struct MissingStringError(StringId);

impl std::fmt::Display for MissingStringError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "missing string {:?}", self.0)
    }
}
