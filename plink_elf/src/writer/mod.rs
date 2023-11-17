mod layout;

pub(crate) use self::layout::WriteLayoutError;

use crate::errors::WriteError;
use crate::ids::{ElfIds, StringIdGetters};
use crate::raw::{
    RawHeader, RawIdentification, RawPadding, RawProgramHeader, RawRel, RawRela, RawSectionHeader,
    RawSymbol, RawType,
};
use crate::writer::layout::{Part, WriteLayout};
use crate::{
    ElfABI, ElfClass, ElfEndian, ElfMachine, ElfObject, ElfSectionContent, ElfSegmentContent,
    ElfSegmentType, ElfSymbolBinding, ElfSymbolDefinition, ElfSymbolType, ElfType,
};
use std::collections::BTreeMap;
use std::io::Write;

pub(crate) struct Writer<'a, I>
where
    I: ElfIds,
    I::StringId: StringIdGetters<I>,
{
    writer: &'a mut dyn Write,
    layout: WriteLayout<I>,
    object: &'a ElfObject<I>,
}

impl<'a, I> Writer<'a, I>
where
    I: ElfIds,
    I::StringId: StringIdGetters<I>,
{
    pub(crate) fn new(
        writer: &'a mut dyn Write,
        object: &'a ElfObject<I>,
    ) -> Result<Self, WriteError> {
        Ok(Self {
            writer,
            layout: WriteLayout::new(object)?,
            object,
        })
    }

    pub(crate) fn write(mut self) -> Result<(), WriteError> {
        let parts = self.layout.parts().iter().cloned().collect::<Vec<_>>();
        for part in &parts {
            match part {
                Part::Identification => self.write_identification()?,
                Part::Header => self.write_header()?,
                Part::SectionHeaders => self.write_section_headers()?,
                Part::ProgramHeaders => self.write_program_headers()?,
                Part::ProgramSection(id) => self.write_program_section(id)?,
                Part::StringTable(id) => self.write_string_table(id)?,
                Part::SymbolTable(id) => self.write_symbol_table(id)?,
                Part::RelocationsTable { id, rela } => self.write_relocations_table(id, *rela)?,
                Part::Padding(_) => self.write_padding(part)?,
            }
        }
        Ok(())
    }

    fn write_identification(&mut self) -> Result<(), WriteError> {
        self.write_raw(RawIdentification {
            magic: [0x7F, b'E', b'L', b'F'],
            class: match self.object.env.class {
                ElfClass::Elf32 => 1,
                ElfClass::Elf64 => 2,
            },
            endian: match self.object.env.endian {
                ElfEndian::Little => 1,
            },
            version: 1,
            abi: match self.object.env.abi {
                ElfABI::SystemV => 0,
            },
            abi_version: match self.object.env.abi {
                ElfABI::SystemV => 0,
            },
            padding: RawPadding,
        })
    }

    fn write_header(&mut self) -> Result<(), WriteError> {
        self.write_raw(RawHeader {
            type_: match self.object.type_ {
                ElfType::Relocatable => 1,
                ElfType::Executable => 2,
                ElfType::SharedObject => 3,
                ElfType::Core => 4,
            },
            machine: match self.object.env.machine {
                ElfMachine::X86 => 3,
                ElfMachine::X86_64 => 62,
            },
            version: 1,
            entry: self.object.entry.map(|n| n.get()).unwrap_or(0),
            program_headers_offset: self.layout.metadata(&Part::ProgramHeaders).offset,
            section_headers_offset: self.layout.metadata(&Part::SectionHeaders).offset,
            flags: 0,
            elf_header_size: self.raw_type_size::<RawIdentification>()
                + self.raw_type_size::<RawHeader>(),
            program_header_size: self.raw_type_size::<RawProgramHeader>(),
            program_header_count: self.object.segments.len() as _,
            section_header_size: self.raw_type_size::<RawSectionHeader>(),
            section_header_count: self.object.sections.len() as _,
            section_names_table_index: self.find_section_names_string_table()?,
        })
    }

    fn write_section_headers(&mut self) -> Result<(), WriteError> {
        for (id, section) in &self.object.sections {
            if let ElfSectionContent::Null = section.content {
                self.write_raw(RawSectionHeader::zero())?;
                continue;
            }

            let metadata = self.layout.metadata_of_section(id);
            self.write_raw(RawSectionHeader {
                name_offset: section.name.offset(),
                type_: match &section.content {
                    ElfSectionContent::Null => unreachable!(),
                    ElfSectionContent::Program(_) => 1,
                    ElfSectionContent::SymbolTable(_) => 2,
                    ElfSectionContent::StringTable(_) => 3,
                    ElfSectionContent::RelocationsTable(_) => 0, // TODO
                    ElfSectionContent::Note(_) => todo!(),
                    ElfSectionContent::Unknown(_) => panic!("unknown section"),
                },
                flags: match &section.content {
                    ElfSectionContent::Program(program) => {
                        (program.perms.write as u64)
                            | (program.perms.read as u64) << 1
                            | (program.perms.execute as u64) << 2
                    }
                    _ => 0,
                },
                memory_address: section.memory_address,
                offset: metadata.offset,
                size: metadata.len,
                link: match &section.content {
                    ElfSectionContent::SymbolTable(table) => {
                        let mut strings = None;
                        for symbol in table.symbols.values() {
                            match strings {
                                Some(existing) if existing == symbol.name.section() => {}
                                Some(_) => return Err(WriteError::InconsistentSymbolNamesTableId),
                                None => strings = Some(symbol.name.section()),
                            }
                        }
                        self.section_idx(strings.expect("no symbols in table")) as _
                    }
                    _ => 0,
                },
                info: match &section.content {
                    // Number of local symbols (aka index of first non-local symbol)
                    ElfSectionContent::SymbolTable(table) => table
                        .symbols
                        .values()
                        .position(|s| s.binding != ElfSymbolBinding::Local)
                        .unwrap_or(table.symbols.len())
                        as _,
                    _ => 0,
                },
                addr_align: 0x1,
                entries_size: 0,
            })?;
        }
        Ok(())
    }

    fn write_program_headers(&mut self) -> Result<(), WriteError> {
        for segment in &self.object.segments {
            let (metadata, section) = match segment.content.as_slice() {
                [ElfSegmentContent::Section(id)] => (
                    self.layout.metadata_of_section(id),
                    self.object.sections.get(id).unwrap(),
                ),
                [ElfSegmentContent::Unknown(_)] => todo!(),
                _ => todo!(),
            };

            self.write_raw(RawProgramHeader {
                type_: match segment.type_ {
                    ElfSegmentType::Null => 0,
                    ElfSegmentType::Load => 1,
                    ElfSegmentType::Dynamic => 2,
                    ElfSegmentType::Interpreter => 3,
                    ElfSegmentType::Note => 4,
                    ElfSegmentType::ProgramHeaderTable => 5,
                    ElfSegmentType::Unknown(_) => panic!("unknown segment"),
                },
                file_offset: metadata.offset,
                virtual_address: section.memory_address,
                reserved: 0,
                file_size: metadata.len,
                memory_size: metadata.len,
                flags: (segment.perms.execute as u32)
                    | (segment.perms.write as u32) << 1
                    | (segment.perms.read as u32) << 2,
                align: 0x1000,
            })?;
        }
        Ok(())
    }

    fn write_string_table(&mut self, id: &I::SectionId) -> Result<(), WriteError> {
        let ElfSectionContent::StringTable(table) = &self.object.sections.get(id).unwrap().content
        else {
            panic!("section {id:?} is not a string table");
        };

        for string in table.all() {
            self.writer.write_all(string.as_bytes())?;
            self.writer.write_all(b"\0")?;
        }
        Ok(())
    }

    fn write_program_section(&mut self, id: &I::SectionId) -> Result<(), WriteError> {
        let ElfSectionContent::Program(program) = &self.object.sections.get(id).unwrap().content
        else {
            panic!("section {id:?} is not a program section");
        };
        self.writer.write_all(&program.raw.0)?;
        Ok(())
    }

    fn write_symbol_table(&mut self, id: &I::SectionId) -> Result<(), WriteError> {
        let ElfSectionContent::SymbolTable(table) = &self.object.sections.get(id).unwrap().content
        else {
            panic!("section {id:?} is not a symbol table")
        };

        for symbol in table.symbols.values() {
            let mut info = 0;
            info |= match symbol.binding {
                ElfSymbolBinding::Local => 0x00,
                ElfSymbolBinding::Global => 0x10,
                ElfSymbolBinding::Weak => 0x20,
                ElfSymbolBinding::Unknown(other) => other << 4,
            };
            info |= match symbol.type_ {
                ElfSymbolType::NoType => 0,
                ElfSymbolType::Object => 1,
                ElfSymbolType::Function => 2,
                ElfSymbolType::Section => 3,
                ElfSymbolType::File => 4,
                ElfSymbolType::Unknown(other) => other & 0xF,
            };
            self.write_raw(RawSymbol {
                name_offset: symbol.name.offset(),
                info,
                reserved: RawPadding,
                definition: match &symbol.definition {
                    ElfSymbolDefinition::Undefined => 0x0000,
                    ElfSymbolDefinition::Absolute => 0xFFF1,
                    ElfSymbolDefinition::Common => 0xFFF2,
                    ElfSymbolDefinition::Section(id) => self.section_idx(id) as _,
                },
                value: symbol.value,
                size: symbol.size,
            })?;
        }

        Ok(())
    }

    fn write_relocations_table(&mut self, id: &I::SectionId, rela: bool) -> Result<(), WriteError> {
        let ElfSectionContent::RelocationsTable(table) =
            &self.object.sections.get(id).unwrap().content
        else {
            panic!("section {id:?} is not a relocation table")
        };

        if rela {
            for _ in 0..table.relocations.len() {
                self.write_raw(RawRela::zero())?;
            }
        } else {
            for _ in 0..table.relocations.len() {
                self.write_raw(RawRel::zero())?;
            }
        }

        // TODO

        Ok(())
    }

    fn write_padding(&mut self, part: &Part<I::SectionId>) -> Result<(), WriteError> {
        let metadata = self.layout.metadata(part);
        let padding = vec![0; metadata.len as usize];
        self.writer.write_all(&padding)?;
        Ok(())
    }

    fn find_section_names_string_table(&self) -> Result<u16, WriteError> {
        let section_ids_to_indices = self
            .object
            .sections
            .keys()
            .enumerate()
            .map(|(idx, id)| (id.clone(), idx as u16))
            .collect::<BTreeMap<_, _>>();

        let mut string_table_section_id = None;
        for section in self.object.sections.values() {
            match &string_table_section_id {
                Some(existing_id) if section.name.section() == existing_id => {}
                Some(_) => return Err(WriteError::InconsistentSectionNamesTableId),
                None => string_table_section_id = Some(section.name.section().clone()),
            }
        }

        string_table_section_id
            .and_then(|id| section_ids_to_indices.get(&id))
            .copied()
            .ok_or(WriteError::MissingSectionNamesTable)
    }

    fn section_idx(&self, id: &I::SectionId) -> usize {
        self.object
            .sections
            .keys()
            .position(|k| k == id)
            .expect("inconsistent section id")
    }

    fn raw_type_size<T: RawType>(&self) -> u16 {
        T::size(self.object.env.class) as _
    }

    fn write_raw<T: RawType>(&mut self, value: T) -> Result<(), WriteError> {
        value.write(self.object.env.class, self.writer)?;
        Ok(())
    }
}
