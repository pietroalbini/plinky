use crate::linker::layout::{LayoutCalculator, LayoutCalculatorError, SectionLayout, SectionMerge};
use crate::linker::relocator::{RelocationError, Relocator};
use crate::linker::strings::{MissingStringError, Strings};
use crate::linker::symbols::{MissingGlobalSymbol, Symbols};
use plink_elf::ids::serial::{SectionId, SerialIds, StringId, SymbolId};
use plink_elf::{
    ElfEndian, ElfObject, ElfProgramSection, ElfRelocation, ElfSectionContent, ElfSymbolDefinition,
};
use std::collections::BTreeMap;
use std::fmt::Display;

#[derive(Debug)]
pub(super) struct Object<L> {
    endian: Option<ElfEndian>,
    program_sections: BTreeMap<SectionId, ProgramSection<L>>,
    strings: Strings,
    symbols: Symbols,
}

impl Object<()> {
    pub(super) fn new() -> Self {
        Self {
            endian: None,
            program_sections: BTreeMap::new(),
            strings: Strings::new(),
            symbols: Symbols::new(),
        }
    }

    pub(super) fn merge_elf(
        &mut self,
        object: ElfObject<SerialIds>,
    ) -> Result<(), ObjectLoadError> {
        self.endian = Some(object.env.endian);

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
        self,
    ) -> Result<(Object<SectionLayout>, Vec<SectionMerge>), LayoutCalculatorError> {
        let mut calculator = LayoutCalculator::new(&self.strings);
        for (id, section) in &self.program_sections {
            calculator.learn_section(
                *id,
                section.name,
                section.program.raw.len(),
                section.program.perms,
            )?;
        }

        let mut layout = calculator.calculate()?;
        let object = Object {
            endian: self.endian,
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
    pub(super) fn relocate(&mut self) -> Result<(), RelocationError> {
        let relocator = Relocator::new(self.endian.unwrap(), &self.program_sections, &self.symbols);
        for section in &mut self.program_sections.values_mut() {
            relocator.relocate(section)?;
        }
        Ok(())
    }

    pub(super) fn take_program_section(&mut self, id: SectionId) -> ElfProgramSection {
        self.program_sections
            .remove(&id)
            .expect("invalid section id")
            .program
    }

    pub(super) fn global_symbol_address(&self, name: &str) -> Result<u64, GetSymbolAddressError> {
        let symbol = self
            .symbols
            .get_global(name)
            .map_err(GetSymbolAddressError::Missing)?;

        match symbol.definition {
            ElfSymbolDefinition::Undefined => Err(GetSymbolAddressError::Undefined(name.into())),
            ElfSymbolDefinition::Absolute => Err(GetSymbolAddressError::NotAnAddress(name.into())),
            ElfSymbolDefinition::Common => todo!(),
            ElfSymbolDefinition::Section(section_id) => {
                let section_offset = self
                    .program_sections
                    .get(&section_id)
                    .expect("invalid section id")
                    .layout
                    .address;
                Ok(section_offset + symbol.value)
            }
        }
    }

    pub(super) fn section_addresses_for_debug_print(&self) -> BTreeMap<SectionId, u64> {
        self.program_sections
            .iter()
            .map(|(id, section)| (*id, section.layout.address))
            .collect()
    }
}

#[derive(Debug)]
pub(crate) struct ProgramSection<L> {
    pub(super) name: StringId,
    pub(super) program: ElfProgramSection,
    pub(super) relocations: Vec<ElfRelocation<SerialIds>>,
    pub(super) layout: L,
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

impl Display for ObjectLoadError {
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

#[derive(Debug)]
pub(crate) enum GetSymbolAddressError {
    Missing(MissingGlobalSymbol),
    Undefined(String),
    NotAnAddress(String),
}

impl std::error::Error for GetSymbolAddressError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            GetSymbolAddressError::Missing(err) => Some(err),
            GetSymbolAddressError::Undefined(_) => None,
            GetSymbolAddressError::NotAnAddress(_) => None,
        }
    }
}

impl Display for GetSymbolAddressError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GetSymbolAddressError::Missing(_) => f.write_str("could not find the symbol"),
            GetSymbolAddressError::Undefined(name) => write!(f, "symbol {name} is undefined"),
            GetSymbolAddressError::NotAnAddress(name) => {
                write!(f, "symbol {name} is not an address")
            }
        }
    }
}
