use crate::linker::strings::{MissingStringError, Strings};
use crate::linker::symbols::Symbols;
use plink_elf::ids::serial::{SectionId, SerialIds, SymbolId};
use plink_elf::{ElfObject, ElfProgramSection, ElfRelocation, ElfSectionContent};
use std::collections::BTreeMap;

#[derive(Debug)]
pub(super) struct Object {
    program_sections: BTreeMap<String, ProgramSection>,
    strings: Strings,
    symbols: Symbols,
}

impl Object {
    pub(super) fn new() -> Self {
        Self {
            program_sections: BTreeMap::new(),
            strings: Strings::new(),
            symbols: Symbols::new(),
        }
    }

    pub(super) fn merge_elf(
        &mut self,
        object: ElfObject<SerialIds>,
    ) -> Result<(), ObjectLoadError> {
        let mut symbol_tables = Vec::new();
        let mut program_sections = Vec::new();
        let mut relocations = BTreeMap::new();

        for (section_id, section) in object.sections.into_iter() {
            match section.content {
                ElfSectionContent::Null => {}
                ElfSectionContent::Program(program) => {
                    program_sections.push((section_id, section.name, program))
                }
                ElfSectionContent::SymbolTable(table) => symbol_tables.push(table),
                ElfSectionContent::StringTable(table) => self.strings.load_table(section_id, table),
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

        // This is loaded after the string tables are loaded by the previous iteration, as we need
        // to resolve the strings as part of symbol loading.
        for table in symbol_tables {
            self.symbols.load_table(table, &self.strings)?;
        }

        for (section_id, section_name, program) in program_sections {
            let section_name = self
                .strings
                .get(section_name)
                .map_err(|e| ObjectLoadError::MissingSectionName(section_id, e))?
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
    UnsupportedUnknownSymbolBinding,
    MissingSectionName(SectionId, MissingStringError),
    MissingSymbolName(SymbolId, MissingStringError),
    DuplicateGlobalSymbol(String),
}

impl std::error::Error for ObjectLoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ObjectLoadError::UnsupportedNotesSection => None,
            ObjectLoadError::UnsupportedUnknownSection => None,
            ObjectLoadError::UnsupportedUnknownSymbolBinding => None,
            ObjectLoadError::MissingSectionName(_, err) => Some(err),
            ObjectLoadError::MissingSymbolName(_, err) => Some(err),
            ObjectLoadError::DuplicateGlobalSymbol(_) => todo!(),
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
            ObjectLoadError::UnsupportedUnknownSymbolBinding => {
                f.write_str("unknown symbol bindings are not supported")
            }
            ObjectLoadError::MissingSectionName(section_id, _) => {
                write!(f, "missing name for section {section_id:?}")
            }
            ObjectLoadError::MissingSymbolName(symbol_id, _) => {
                write!(f, "missing name for symbol {symbol_id:?}")
            }
            ObjectLoadError::DuplicateGlobalSymbol(symbol) => {
                write!(f, "duplicate global symbol {symbol}")
            }
        }
    }
}
