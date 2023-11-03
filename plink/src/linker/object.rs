use crate::linker::layout::{LayoutCalculator, LayoutCalculatorError, SectionLayout, SectionMerge};
use crate::linker::strings::{MissingStringError, Strings};
use crate::linker::symbols::Symbols;
use plink_elf::ids::serial::{SectionId, SerialIds, StringId, SymbolId};
use plink_elf::{ElfObject, ElfProgramSection, ElfRelocation, ElfSectionContent};
use std::collections::BTreeMap;

#[derive(Debug)]
pub(super) struct Object<L> {
    program_sections: BTreeMap<SectionId, ProgramSection<L>>,
    strings: Strings,
    symbols: Symbols,
}

impl Object<()> {
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

        for (section_id, name, program) in program_sections {
            let relocations = relocations.remove(&section_id).unwrap_or_else(Vec::new);
            self.program_sections.insert(
                section_id,
                ProgramSection {
                    name,
                    program,
                    relocations,
                    layout: (),
                },
            );
        }

        Ok(())
    }

    pub(super) fn calculate_layout(
        mut self,
    ) -> Result<(Object<SectionLayout>, Vec<SectionMerge>), LayoutCalculatorError> {
        let mut calculator = LayoutCalculator::new(&self.strings);
        for (id, section) in &self.program_sections {
            calculator.learn_section(*id, section.name, section.program.raw.len())?;
        }

        let mut layout = calculator.calculate();
        let object = Object {
            program_sections: self
                .program_sections
                .into_iter()
                .map(|(id, section)| {
                    (
                        id,
                        ProgramSection {
                            name: section.name,
                            program: section.program,
                            relocations: section.relocations,
                            layout: layout.sections.remove(&id).unwrap(),
                        },
                    )
                })
                .collect(),
            strings: self.strings,
            symbols: self.symbols,
        };

        Ok((object, layout.merges))
    }
}

impl Object<SectionLayout> {
    pub(super) fn section_addresses_for_debug_print(&self) -> BTreeMap<SectionId, u64> {
        self.program_sections
            .iter()
            .map(|(id, section)| (*id, section.layout.address))
            .collect()
    }
}

#[derive(Debug)]
pub(crate) struct ProgramSection<L> {
    name: StringId,
    program: ElfProgramSection,
    relocations: Vec<ElfRelocation<SerialIds>>,
    layout: L,
}

#[derive(Debug)]
pub(crate) enum ObjectLoadError {
    UnsupportedNotesSection,
    UnsupportedUnknownSection,
    UnsupportedUnknownSymbolBinding,
    MissingSymbolName(SymbolId, MissingStringError),
    DuplicateGlobalSymbol(String),
}

impl std::error::Error for ObjectLoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ObjectLoadError::UnsupportedNotesSection => None,
            ObjectLoadError::UnsupportedUnknownSection => None,
            ObjectLoadError::UnsupportedUnknownSymbolBinding => None,
            ObjectLoadError::MissingSymbolName(_, err) => Some(err),
            ObjectLoadError::DuplicateGlobalSymbol(_) => None,
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
            ObjectLoadError::MissingSymbolName(symbol_id, _) => {
                write!(f, "missing name for symbol {symbol_id:?}")
            }
            ObjectLoadError::DuplicateGlobalSymbol(symbol) => {
                write!(f, "duplicate global symbol {symbol}")
            }
        }
    }
}
