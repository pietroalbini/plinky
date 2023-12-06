use crate::repr::layout::{LayoutCalculator, LayoutCalculatorError, SectionLayout, SectionMerge};
use crate::repr::relocator::{RelocationError, Relocator};
use crate::repr::strings::{MissingStringError, Strings};
use crate::repr::symbols::{MissingGlobalSymbol, Symbols};
use plink_elf::ids::serial::{SectionId, SerialIds, StringId, SymbolId};
use plink_elf::{
    ElfEndian, ElfObject, ElfPermissions, ElfRelocation, ElfSectionContent, ElfSymbolDefinition,
    RawBytes,
};
use plink_macros::{Display, Error};
use std::collections::BTreeMap;

#[derive(Debug)]
pub(crate) struct Object<L> {
    endian: Option<ElfEndian>,
    sections: BTreeMap<SectionId, Section<L>>,
    strings: Strings,
    symbols: Symbols,
}

impl Object<()> {
    pub(crate) fn new() -> Self {
        Self {
            endian: None,
            sections: BTreeMap::new(),
            strings: Strings::new(),
            symbols: Symbols::new(),
        }
    }

    pub(crate) fn merge_elf(
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
                ElfSectionContent::Uninitialized(uninit) => {
                    self.sections.insert(
                        section_id,
                        Section {
                            name: section.name,
                            perms: uninit.perms,
                            content: SectionContent::Uninitialized(UninitializedSection {
                                len: uninit.len,
                            }),
                            layout: (),
                        },
                    );
                }
                ElfSectionContent::SymbolTable(table) => symbol_tables.push(table),
                ElfSectionContent::StringTable(table) => self.strings.load_table(section_id, table),
                ElfSectionContent::RelocationsTable(table) => {
                    relocations.insert(table.applies_to_section, table.relocations);
                }
                ElfSectionContent::Note(table) => {
                    for note in table.notes {
                        match note {
                            plink_elf::ElfNote::Unknown(unknown) => {
                                return Err(ObjectLoadError::UnsupportedUnknownNote {
                                    name: unknown.name,
                                    type_: unknown.type_,
                                })
                            }
                        }
                    }
                }
                ElfSectionContent::Unknown(unknown) => {
                    return Err(ObjectLoadError::UnsupportedUnknownSection { id: unknown.id });
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
            self.sections.insert(
                section_id,
                Section {
                    name,
                    perms: program.perms,
                    content: SectionContent::Data(DataSection {
                        bytes: program.raw,
                        relocations,
                    }),
                    layout: (),
                },
            );
        }

        Ok(())
    }

    pub(crate) fn calculate_layout(
        self,
    ) -> Result<(Object<SectionLayout>, Vec<SectionMerge>), LayoutCalculatorError> {
        let mut calculator = LayoutCalculator::new(&self.strings);
        for (id, section) in &self.sections {
            calculator.learn_section(
                *id,
                section.name,
                match &section.content {
                    SectionContent::Data(data) => data.bytes.len(),
                    SectionContent::Uninitialized(uninit) => uninit.len as usize,
                },
                section.perms,
            )?;
        }

        let mut layout = calculator.calculate()?;
        let object = Object {
            endian: self.endian,
            sections: self
                .sections
                .into_iter()
                .map(|(id, section)| {
                    (
                        id,
                        Section {
                            name: section.name,
                            perms: section.perms,
                            content: section.content,
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
    pub(crate) fn relocate(&mut self) -> Result<(), RelocationError> {
        let relocator = Relocator::new(self.section_layouts(), &self.symbols);
        for (id, section) in &mut self.sections.iter_mut() {
            match &mut section.content {
                SectionContent::Data(data) => relocator.relocate(*id, data)?,
                SectionContent::Uninitialized(_) => {}
            }
        }
        Ok(())
    }

    pub(crate) fn take_section(&mut self, id: SectionId) -> Section<SectionLayout> {
        self.sections.remove(&id).expect("invalid section id")
    }

    pub(crate) fn global_symbol_address(&self, name: &str) -> Result<u64, GetSymbolAddressError> {
        let symbol = self.symbols.get_global(name)?;

        match symbol.definition {
            ElfSymbolDefinition::Undefined => Err(GetSymbolAddressError::Undefined(name.into())),
            ElfSymbolDefinition::Absolute => Err(GetSymbolAddressError::NotAnAddress(name.into())),
            ElfSymbolDefinition::Common => todo!(),
            ElfSymbolDefinition::Section(section_id) => {
                let section_offset = self
                    .sections
                    .get(&section_id)
                    .expect("invalid section id")
                    .layout
                    .address;
                Ok(section_offset + symbol.value)
            }
        }
    }

    pub(crate) fn section_layouts(&self) -> impl Iterator<Item = (SectionId, &'_ SectionLayout)> {
        self.sections
            .iter()
            .map(|(id, section)| (*id, &section.layout))
    }
}

#[derive(Debug)]
pub(crate) struct Section<L> {
    pub(crate) name: StringId,
    pub(crate) perms: ElfPermissions,
    pub(crate) content: SectionContent,
    pub(crate) layout: L,
}

#[derive(Debug)]
pub(crate) enum SectionContent {
    Data(DataSection),
    Uninitialized(UninitializedSection),
}

#[derive(Debug)]
pub(crate) struct DataSection {
    pub(crate) bytes: RawBytes,
    pub(crate) relocations: Vec<ElfRelocation<SerialIds>>,
}

#[derive(Debug)]
pub(crate) struct UninitializedSection {
    pub(crate) len: u64,
}

#[derive(Debug, Error, Display)]
pub(crate) enum ObjectLoadError {
    #[display("unsupported note with name {name} and type {type_}")]
    UnsupportedUnknownNote { name: String, type_: u32 },
    #[display("unknown section with type {id:#x?} is not supported")]
    UnsupportedUnknownSection { id: u32 },
    #[display("unknown symbol bindings are not supported")]
    UnsupportedUnknownSymbolBinding,
    #[display("missing name for symbol {f0:?}")]
    MissingSymbolName(SymbolId, #[source] MissingStringError),
    #[display("duplicate global symbol {f0}")]
    DuplicateGlobalSymbol(String),
}

#[derive(Debug, Error, Display)]
pub(crate) enum GetSymbolAddressError {
    #[display("could not find the symbol")]
    Missing(#[from] MissingGlobalSymbol),
    #[display("symbol {f0} is undefined")]
    Undefined(String),
    #[display("symbol {f0} is not an address")]
    NotAnAddress(String),
}
