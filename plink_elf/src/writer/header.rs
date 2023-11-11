use super::replacements::Replacement;
use crate::errors::WriteError;
use crate::ids::{ElfIds, StringIdGetters};
use crate::writer::Writer;
use crate::{ElfABI, ElfClass, ElfEndian, ElfMachine, ElfType};
use std::collections::BTreeMap;

impl<'a, I> Writer<'a, I>
where
    I: ElfIds,
    I::StringId: StringIdGetters<I>,
{
    pub(super) fn write_header(&mut self) -> Result<(), WriteError> {
        self.write_magic()?;

        self.cursor.write_u8(match self.object.env.class {
            ElfClass::Elf32 => 1,
            ElfClass::Elf64 => 2,
        })?;
        self.cursor.write_u8(match self.object.env.endian {
            ElfEndian::Little => 1,
        })?;
        self.cursor.write_u8(1)?; // Version
        match self.object.env.abi {
            ElfABI::SystemV => {
                self.cursor.write_u8(0)?; // ABI
                self.cursor.write_u8(0)?; // Version
            }
        }

        // Padding bytes:
        for _ in 0..7 {
            self.cursor.write_u8(0)?;
        }

        self.cursor.write_u16(match self.object.type_ {
            ElfType::Relocatable => 1,
            ElfType::Executable => 2,
            ElfType::SharedObject => 3,
            ElfType::Core => 4,
        })?;
        self.cursor.write_u16(match self.object.env.machine {
            ElfMachine::X86 => 3,
            ElfMachine::X86_64 => 62,
        })?;
        self.cursor.write_u32(1)?; // Version
        self.cursor.write_usize(match self.object.entry {
            Some(entry) => entry.get(),
            None => 0,
        })?;

        self.replacements
            .write(&mut self.cursor, Replacement::ProgramHeaderOffset)?;
        self.replacements
            .write(&mut self.cursor, Replacement::SectionHeaderOffset)?;

        self.cursor.write_u32(0)?; // Flags

        self.replacements
            .write(&mut self.cursor, Replacement::HeaderSize)?;

        self.replacements
            .write(&mut self.cursor, Replacement::ProgramHeaderEntrySize)?;
        self.cursor.write_u16(self.object.segments.len() as _)?;

        self.replacements
            .write(&mut self.cursor, Replacement::SectionHeaderEntrySize)?;
        self.cursor.write_u16(self.object.sections.len() as _)?;

        self.cursor
            .write_u16(self.find_section_names_string_table()?)?;

        Ok(())
    }

    fn write_magic(&mut self) -> Result<(), WriteError> {
        self.cursor.write_u8(0x7F)?;
        self.cursor.write_u8(b'E')?;
        self.cursor.write_u8(b'L')?;
        self.cursor.write_u8(b'F')?;
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
}
