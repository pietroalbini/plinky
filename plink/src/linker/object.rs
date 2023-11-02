use crate::linker::LinkerError;
use plink_elf::ids::serial::{SectionId, SerialIds, StringId, SymbolId};
use plink_elf::ids::StringIdGetters;
use plink_elf::{
    ElfObject, ElfProgramSection, ElfRelocation, ElfSectionContent, ElfStringTable, ElfSymbol,
};
use std::collections::BTreeMap;

#[derive(Debug)]
pub(super) struct Object {
    program_sections: BTreeMap<String, ProgramSection>,
    string_tables: BTreeMap<SectionId, ElfStringTable>,
    symbols: BTreeMap<SymbolId, ElfSymbol<SerialIds>>,
}

impl Object {
    pub(super) fn new() -> Self {
        Self {
            program_sections: BTreeMap::new(),
            string_tables: BTreeMap::new(),
            symbols: BTreeMap::new(),
        }
    }

    pub(super) fn merge_elf(
        &mut self,
        object: ElfObject<SerialIds>,
    ) -> Result<(), ObjectLoadError> {
        let mut program_sections = Vec::new();
        let mut relocations = BTreeMap::new();

        for (section_id, section) in object.sections.into_iter() {
            match section.content {
                ElfSectionContent::Null => {}
                ElfSectionContent::Program(program) => {
                    program_sections.push((section_id, section.name, program))
                }
                ElfSectionContent::SymbolTable(table) => {
                    for (symbol_id, symbol) in table.symbols.into_iter() {
                        self.symbols.insert(symbol_id, symbol);
                    }
                }
                ElfSectionContent::StringTable(table) => {
                    self.string_tables.insert(section_id, table);
                }
                ElfSectionContent::RelocationsTable(table) => {
                    relocations.insert(table.applies_to_section, table.relocations);
                }
                ElfSectionContent::Note(_) => {
                    return Err(ObjectLoadError::UnsupportedNotesSection);
                }
                ElfSectionContent::Unknown(_) => {
                    return Err(ObjectLoadError::UnsupportedUnknownSection);
                }
            }
        }

        for (section_id, section_name, program) in program_sections {
            let section_name = self
                .get_string(section_name)
                .map_err(|e| ObjectLoadError::Generic(Box::new(e)))?
                .to_string();
            let relocations = relocations.remove(&section_id).unwrap_or_else(Vec::new);
            match self.program_sections.get_mut(&section_name) {
                Some(_) => todo!(),
                None => {
                    self.program_sections.insert(
                        section_name,
                        ProgramSection {
                            program,
                            relocations,
                        },
                    );
                }
            }
        }

        Ok(())
    }

    fn get_string(&self, id: StringId) -> Result<&str, LinkerError> {
        self.string_tables
            .get(id.section())
            .and_then(|table| table.get(id.offset()))
            .ok_or(LinkerError::MissingString(id))
    }
}

#[derive(Debug)]
struct ProgramSection {
    program: ElfProgramSection,
    relocations: Vec<ElfRelocation<SerialIds>>,
}

#[derive(Debug)]
pub(crate) enum ObjectLoadError {
    UnsupportedNotesSection,
    UnsupportedUnknownSection,
    Generic(Box<LinkerError>),
}

impl std::error::Error for ObjectLoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ObjectLoadError::UnsupportedNotesSection => None,
            ObjectLoadError::UnsupportedUnknownSection => None,
            ObjectLoadError::Generic(err) => Some(err),
        }
    }
}

impl std::fmt::Display for ObjectLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ObjectLoadError::UnsupportedNotesSection => {
                f.write_str("note sections are not supported")
            }
            ObjectLoadError::UnsupportedUnknownSection => {
                f.write_str("unknown sections are not supported")
            }
            ObjectLoadError::Generic(_) => f.write_str("error happened during loading"),
        }
    }
}
