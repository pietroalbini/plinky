pub mod layout;

pub(crate) use self::layout::LayoutError;

use crate::errors::WriteError;
use crate::ids::{ElfIds, StringIdGetters};
use crate::raw::{
    RawGroupFlags, RawHashHeader, RawHeader, RawHeaderFlags, RawIdentification, RawProgramHeader,
    RawProgramHeaderFlags, RawRel, RawRela, RawSectionHeader, RawSectionHeaderFlags, RawSymbol,
};
use crate::writer::layout::{Layout, Part};
use crate::{
    ElfABI, ElfClass, ElfDeduplication, ElfDynamicDirective, ElfEndian, ElfMachine, ElfObject,
    ElfPLTRelocationsMode, ElfPermissions, ElfProgramSection, ElfRelocationType, ElfSectionContent,
    ElfSegmentContent, ElfSegmentType, ElfSymbolBinding, ElfSymbolDefinition, ElfSymbolTable,
    ElfSymbolType, ElfSymbolVisibility, ElfType,
};
use plinky_utils::bitfields::Bitfield;
use plinky_utils::raw_types::{RawPadding, RawType};
use std::collections::BTreeMap;
use std::io::Write;

pub struct Writer<'a, I>
where
    I: ElfIds,
    I::StringId: StringIdGetters<I>,
{
    writer: &'a mut dyn Write,
    layout: Layout<I>,
    object: &'a ElfObject<I>,
}

impl<'a, I> Writer<'a, I>
where
    I: ElfIds,
    I::StringId: StringIdGetters<I>,
{
    pub fn new(writer: &'a mut dyn Write, object: &'a ElfObject<I>) -> Result<Self, WriteError<I>> {
        Ok(Self { writer, layout: Layout::new(object)?, object })
    }

    pub fn write(mut self) -> Result<(), WriteError<I>> {
        let parts = self.layout.parts().to_vec();
        for part in &parts {
            match part {
                Part::Header => {
                    self.write_identification()?;
                    self.write_header()?;
                }
                Part::SectionHeaders => self.write_section_headers()?,
                Part::ProgramHeaders => self.write_program_headers()?,
                Part::ProgramSection(id) => self.write_program_section(id)?,
                Part::StringTable(id) => self.write_string_table(id)?,
                Part::SymbolTable(id) => self.write_symbol_table(id)?,
                Part::Rela(id) => self.write_relocations_table(id, true)?,
                Part::Rel(id) => self.write_relocations_table(id, false)?,
                Part::Group(id) => self.write_group(id)?,
                Part::Hash(id) => self.write_hash(id)?,
                Part::Dynamic(id) => self.write_dynamic(id)?,
                Part::Padding { .. } => self.write_padding(part)?,
            }
        }
        Ok(())
    }

    fn write_identification(&mut self) -> Result<(), WriteError<I>> {
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

    fn write_header(&mut self) -> Result<(), WriteError<I>> {
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
            program_headers_offset: self
                .layout
                .metadata(&Part::ProgramHeaders)
                .file
                .as_ref()
                .unwrap()
                .offset,
            section_headers_offset: self
                .layout
                .metadata(&Part::SectionHeaders)
                .file
                .as_ref()
                .unwrap()
                .offset,
            flags: RawHeaderFlags::zero(),
            elf_header_size: self.raw_type_size::<RawIdentification>()
                + self.raw_type_size::<RawHeader>(),
            program_header_size: self.raw_type_size::<RawProgramHeader>(),
            program_header_count: self.object.segments.len() as _,
            section_header_size: self.raw_type_size::<RawSectionHeader>(),
            section_header_count: self.object.sections.len() as _,
            section_names_table_index: self.find_section_names_string_table()?,
        })
    }

    fn write_section_headers(&mut self) -> Result<(), WriteError<I>> {
        for (id, section) in &self.object.sections {
            let type_ = match &section.content {
                ElfSectionContent::Null => {
                    self.write_raw(RawSectionHeader::zero())?;
                    continue;
                }

                ElfSectionContent::Uninitialized(uninit) => {
                    self.write_raw(RawSectionHeader {
                        name_offset: section.name.offset(),
                        type_: 8,
                        flags: self.perms_to_section_flags(&uninit.perms),
                        memory_address: section.memory_address,
                        offset: 0,
                        size: uninit.len,
                        link: 0,
                        info: 0,
                        addr_align: 0x1,
                        entries_size: 0,
                    })?;
                    continue;
                }

                ElfSectionContent::Program(_) => 1,
                ElfSectionContent::SymbolTable(ElfSymbolTable { dynsym: false, .. }) => 2,
                ElfSectionContent::SymbolTable(ElfSymbolTable { dynsym: true, .. }) => 11,
                ElfSectionContent::StringTable(_) => 3,
                ElfSectionContent::Hash(_) => 5,
                ElfSectionContent::Dynamic(_) => 6,
                ElfSectionContent::Note(_) => todo!(),
                ElfSectionContent::Unknown(_) => panic!("unknown section"),
                ElfSectionContent::RelocationsTable(_) => self
                    .layout
                    .parts()
                    .iter()
                    .filter_map(|part| match part {
                        Part::Rel(part_id) if part_id == id => Some(9),
                        Part::Rela(part_id) if part_id == id => Some(4),
                        _ => None,
                    })
                    .next()
                    .expect("relocations table not in layout"),
                ElfSectionContent::Group(_) => 17,
            };

            let mut flags = match &section.content {
                ElfSectionContent::Program(p) => {
                    let mut flags = self.perms_to_section_flags(&p.perms);
                    match p.deduplication {
                        ElfDeduplication::Disabled => {}
                        ElfDeduplication::ZeroTerminatedStrings => {
                            flags.merge = true;
                            flags.strings = true;
                        }
                        ElfDeduplication::FixedSizeChunks { .. } => {
                            flags.merge = true;
                        }
                    }
                    flags
                }
                ElfSectionContent::RelocationsTable(_) => {
                    RawSectionHeaderFlags { info_link: true, ..RawSectionHeaderFlags::zero() }
                }
                _ => RawSectionHeaderFlags::zero(),
            };
            if section.part_of_group {
                flags.group = true;
            }

            let metadata = self.layout.metadata_of_section(id).file.as_ref().unwrap();
            self.write_raw(RawSectionHeader {
                name_offset: section.name.offset(),
                type_,
                flags,
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
                    ElfSectionContent::RelocationsTable(table) => {
                        self.section_idx(&table.symbol_table) as _
                    }
                    ElfSectionContent::Hash(hash) => self.section_idx(&hash.symbol_table) as _,
                    ElfSectionContent::Group(group) => self.section_idx(&group.symbol_table) as _,
                    ElfSectionContent::Dynamic(dynamic) => {
                        self.section_idx(&dynamic.string_table) as _
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
                    ElfSectionContent::RelocationsTable(table) => {
                        self.section_idx(&table.applies_to_section) as _
                    }
                    ElfSectionContent::Group(group) => {
                        let ElfSectionContent::SymbolTable(symbol_table) =
                            &self.object.sections.get(&group.symbol_table).unwrap().content
                        else {
                            return Err(WriteError::WrongSectionTypeForGroupSymbolTable {
                                group: id.clone(),
                                symbol_table: group.symbol_table.clone(),
                            });
                        };
                        symbol_table
                            .symbols
                            .iter()
                            .position(|(id, _)| *id == group.signature)
                            .ok_or_else(|| WriteError::MissingGroupSignature {
                                group: id.clone(),
                                signature: group.signature.clone(),
                            })? as _
                    }
                    _ => 0,
                },
                addr_align: 0x1,
                entries_size: match &section.content {
                    ElfSectionContent::Program(ElfProgramSection {
                        deduplication: ElfDeduplication::FixedSizeChunks { size },
                        ..
                    }) => size.get(),
                    ElfSectionContent::Program(ElfProgramSection {
                        deduplication: ElfDeduplication::ZeroTerminatedStrings,
                        ..
                    }) => 1,
                    ElfSectionContent::SymbolTable(_) => {
                        RawSymbol::size(self.object.env.class) as _
                    }
                    ElfSectionContent::RelocationsTable(r) => {
                        if r.relocations.first().map(|f| f.addend.is_some()).unwrap_or(false) {
                            RawRela::size(self.object.env.class) as _
                        } else {
                            RawRel::size(self.object.env.class) as _
                        }
                    }
                    _ => 0,
                },
            })?;
        }
        Ok(())
    }

    fn write_program_headers(&mut self) -> Result<(), WriteError<I>> {
        for segment in &self.object.segments {
            let (file_offset, file_size, virtual_address, memory_size) = match &segment.content {
                ElfSegmentContent::Sections(section_ids) => {
                    let mut section_ids = section_ids.iter();
                    let first_section_id = section_ids.next().unwrap();
                    let first_section = self.object.sections.get(first_section_id).unwrap();

                    let (file_offset, mut file_size, mut memory_size) = match &first_section.content
                    {
                        ElfSectionContent::Uninitialized(uninit) => (0, 0, uninit.len),
                        content => {
                            let metadata = self.layout.metadata_of_section(first_section_id).file.as_ref().unwrap();
                            (
                                metadata.offset,
                                metadata.len,
                                content.content_size(self.object.env.class) as u64,
                            )
                        }
                    };

                    let mut expected_next_file_offset = file_offset + file_size;
                    for section_id in section_ids {
                        let section = self.object.sections.get(section_id).unwrap();
                        match &section.content {
                            ElfSectionContent::Uninitialized(uninit) => {
                                if expected_next_file_offset != 0 {
                                    panic!("mixed uninitialized section with program sections in segment");
                                }
                                memory_size += uninit.len;
                            }
                            content => {
                                let metadata = self.layout.metadata_of_section(section_id).file.as_ref().unwrap();
                                if metadata.offset != expected_next_file_offset {
                                    panic!("sections in segment are not adjacent");
                                }
                                expected_next_file_offset += metadata.len;
                                file_size += metadata.len;
                                memory_size += content.content_size(self.object.env.class) as u64;
                            }
                        }
                    }
                    (file_offset, file_size, first_section.memory_address, memory_size)
                }
                ElfSegmentContent::ElfHeader => {
                    self.layout.metadata(&Part::Header).segment_bounds()
                }
                ElfSegmentContent::ProgramHeader => {
                    self.layout.metadata(&Part::ProgramHeaders).segment_bounds()
                }
                ElfSegmentContent::Unknown(unknown) => (
                    unknown.file_offset,
                    unknown.file_size,
                    unknown.virtual_address,
                    unknown.memory_size,
                ),
                ElfSegmentContent::Empty => (0, 0, 0, 0),
            };

            self.write_raw(RawProgramHeader {
                type_: match segment.type_ {
                    ElfSegmentType::Null => 0,
                    ElfSegmentType::Load => 1,
                    ElfSegmentType::Dynamic => 2,
                    ElfSegmentType::Interpreter => 3,
                    ElfSegmentType::Note => 4,
                    ElfSegmentType::ProgramHeaderTable => 6,
                    ElfSegmentType::GnuStack => 0x6474e551,
                    ElfSegmentType::GnuRelRO => 0x6474e552,
                    ElfSegmentType::Unknown(_) => panic!("unknown segment"),
                },
                file_offset,
                virtual_address,
                reserved: 0,
                file_size,
                memory_size,
                flags: RawProgramHeaderFlags {
                    execute: segment.perms.execute,
                    write: segment.perms.write,
                    read: segment.perms.read,
                },
                align: segment.align,
            })?;
        }
        Ok(())
    }

    fn write_string_table(&mut self, id: &I::SectionId) -> Result<(), WriteError<I>> {
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

    fn write_program_section(&mut self, id: &I::SectionId) -> Result<(), WriteError<I>> {
        let ElfSectionContent::Program(program) = &self.object.sections.get(id).unwrap().content
        else {
            panic!("section {id:?} is not a program section");
        };
        self.writer.write_all(&program.raw.0)?;
        Ok(())
    }

    fn write_symbol_table(&mut self, id: &I::SectionId) -> Result<(), WriteError<I>> {
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
                other: match &symbol.visibility {
                    ElfSymbolVisibility::Default => 0,
                    ElfSymbolVisibility::Hidden => 2,
                    ElfSymbolVisibility::Protected => 3,
                    ElfSymbolVisibility::Exported => 4,
                    ElfSymbolVisibility::Singleton => 5,
                    ElfSymbolVisibility::Eliminate => 6,
                },
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

    fn write_relocations_table(
        &mut self,
        id: &I::SectionId,
        rela: bool,
    ) -> Result<(), WriteError<I>> {
        let ElfSectionContent::RelocationsTable(table) =
            &self.object.sections.get(id).unwrap().content
        else {
            panic!("section {id:?} is not a relocation table")
        };

        let ElfSectionContent::SymbolTable(symbol_table) = &self
            .object
            .sections
            .get(&table.symbol_table)
            .ok_or_else(|| WriteError::MissingSymbolTableForRelocations {
                symbol_table: table.symbol_table.clone(),
                relocations_table: id.clone(),
            })?
            .content
        else {
            panic!("section {id:?} is not a symbol table")
        };

        for (idx, relocation) in table.relocations.iter().enumerate() {
            let relocation_type = match relocation.relocation_type {
                ElfRelocationType::X86_None => 0,
                ElfRelocationType::X86_32 => 1,
                ElfRelocationType::X86_PC32 => 2,
                ElfRelocationType::X86_GOT32 => 3,
                ElfRelocationType::X86_PLT32 => 4,
                ElfRelocationType::X86_COPY => 5,
                ElfRelocationType::X86_GLOB_DAT => 6,
                ElfRelocationType::X86_JMP_Slot => 7,
                ElfRelocationType::X86_Relative => 8,
                ElfRelocationType::X86_GOTOff => 9,
                ElfRelocationType::X86_GOTPC => 10,
                ElfRelocationType::X86_GOT32X => 11,
                ElfRelocationType::X86_64_None => 0,
                ElfRelocationType::X86_64_64 => 1,
                ElfRelocationType::X86_64_PC32 => 2,
                ElfRelocationType::X86_64_GOT32 => 3,
                ElfRelocationType::X86_64_PLT32 => 4,
                ElfRelocationType::X86_64_Copy => 5,
                ElfRelocationType::X86_64_GlobDat => 6,
                ElfRelocationType::X86_64_JumpSlot => 7,
                ElfRelocationType::X86_64_Relative => 8,
                ElfRelocationType::X86_64_GOTPCRel => 9,
                ElfRelocationType::X86_64_32 => 10,
                ElfRelocationType::X86_64_32S => 11,
                ElfRelocationType::X86_64_16 => 12,
                ElfRelocationType::X86_64_PC16 => 13,
                ElfRelocationType::X86_64_8 => 14,
                ElfRelocationType::X86_64_PC8 => 15,
                ElfRelocationType::X86_64_DTPMod64 => 16,
                ElfRelocationType::X86_64_DTPOff64 => 17,
                ElfRelocationType::X86_64_TPOff64 => 18,
                ElfRelocationType::X86_64_TLSGD => 19,
                ElfRelocationType::X86_64_TLSLD => 20,
                ElfRelocationType::X86_64_DTPOff32 => 21,
                ElfRelocationType::X86_64_GOTTPOff => 22,
                ElfRelocationType::X86_64_TPOff32 => 23,
                ElfRelocationType::X86_64_PC64 => 24,
                ElfRelocationType::X86_64_GOTOff64 => 25,
                ElfRelocationType::X86_64_GOTPC32 => 26,
                ElfRelocationType::X86_64_Size32 => 32,
                ElfRelocationType::X86_64_Size64 => 33,
                ElfRelocationType::X86_64_GOTPC32_TLSDesc => 34,
                ElfRelocationType::X86_64_TLSDescCall => 35,
                ElfRelocationType::X86_64_TLSDesc => 36,
                ElfRelocationType::X86_64_IRelative => 37,
                ElfRelocationType::X86_64_IRelative64 => 38,
                ElfRelocationType::X86_64_GOTPCRelX => 41,
                ElfRelocationType::X86_64_Rex_GOTPCRelX => 42,
                ElfRelocationType::X86_64_Code_4_GOTPCRelX => 43,
                ElfRelocationType::X86_64_Code_4_GOTPCOff => 44,
                ElfRelocationType::X86_64_Code_4_GOTPC32_TLSDesc => 45,
                ElfRelocationType::X86_64_Code_5_GOTPCRelX => 46,
                ElfRelocationType::X86_64_Code_5_GOTPCOff => 47,
                ElfRelocationType::X86_64_Code_5_GOTPC32_TLSDesc => 48,
                ElfRelocationType::X86_64_Code_6_GOTPCRelX => 49,
                ElfRelocationType::X86_64_Code_6_GOTPCOff => 50,
                ElfRelocationType::X86_64_Code_6_GOTPC32_TLSDesc => 51,
                ElfRelocationType::Unknown(other) => other as u64,
            };
            let symbol =
                symbol_table.symbols.keys().position(|id| *id == relocation.symbol).ok_or_else(
                    || WriteError::MissingSymbolInRelocation {
                        symbol_id: relocation.symbol.clone(),
                        relocations_table: id.clone(),
                        relocation_idx: idx,
                    },
                )? as u64;
            let info = match self.object.env.class {
                ElfClass::Elf32 => relocation_type | (symbol << 8),
                ElfClass::Elf64 => relocation_type | (symbol << 32),
            };

            if rela {
                self.write_raw(RawRela {
                    offset: relocation.offset,
                    info,
                    addend: relocation.addend.expect("rela relocation without addend"),
                })?;
            } else {
                self.write_raw(RawRel { offset: relocation.offset, info })?;
            }
        }

        Ok(())
    }

    fn write_group(&mut self, id: &I::SectionId) -> Result<(), WriteError<I>> {
        let ElfSectionContent::Group(group) = &self.object.sections.get(id).unwrap().content else {
            panic!("section {id:?} is not a group");
        };
        self.write_raw(RawGroupFlags { comdat: group.comdat })?;
        for section in &group.sections {
            self.write_raw(self.section_idx(section) as u32)?;
        }
        Ok(())
    }

    fn write_hash(&mut self, id: &I::SectionId) -> Result<(), WriteError<I>> {
        let ElfSectionContent::Hash(hash) = &self.object.sections.get(id).unwrap().content else {
            panic!("section {id:?} is not a hash");
        };
        self.write_raw(RawHashHeader {
            bucket_count: hash.buckets.len().try_into().expect("too many buckets"),
            chain_count: hash.chain.len().try_into().expect("too many chain elements"),
        })?;
        for entry in &hash.buckets {
            self.write_raw(*entry)?;
        }
        for entry in &hash.chain {
            self.write_raw(*entry)?;
        }
        Ok(())
    }

    fn write_dynamic(&mut self, id: &I::SectionId) -> Result<(), WriteError<I>> {
        let ElfSectionContent::Dynamic(dynamic) = &self.object.sections.get(id).unwrap().content
        else {
            panic!("section {id:?} is not a dynamic section");
        };

        for directive in &dynamic.directives {
            let (tag, value) = match directive {
                ElfDynamicDirective::Null => (0, 0),
                ElfDynamicDirective::Needed { string_table_offset } => (1, *string_table_offset),
                ElfDynamicDirective::PLTRelocationsSize { bytes } => (2, *bytes),
                ElfDynamicDirective::PLTGOT { address } => (3, *address),
                ElfDynamicDirective::Hash { address } => (4, *address),
                ElfDynamicDirective::GnuHash { address } => (0x6ffffef5, *address),
                ElfDynamicDirective::StringTable { address } => (5, *address),
                ElfDynamicDirective::SymbolTable { address } => (6, *address),
                ElfDynamicDirective::Rela { address } => (7, *address),
                ElfDynamicDirective::RelaSize { bytes } => (8, *bytes),
                ElfDynamicDirective::RelaEntrySize { bytes } => (9, *bytes),
                ElfDynamicDirective::StringTableSize { bytes } => (10, *bytes),
                ElfDynamicDirective::SymbolTableEntrySize { bytes } => (11, *bytes),
                ElfDynamicDirective::InitFunction { address } => (12, *address),
                ElfDynamicDirective::FiniFunction { address } => (13, *address),
                ElfDynamicDirective::SharedObjectName { string_table_offset } => {
                    (14, *string_table_offset)
                }
                ElfDynamicDirective::RuntimePath { string_table_offset } => {
                    (15, *string_table_offset)
                }
                ElfDynamicDirective::Symbolic => (16, 0),
                ElfDynamicDirective::Rel { address } => (17, *address),
                ElfDynamicDirective::RelSize { bytes } => (18, *bytes),
                ElfDynamicDirective::RelEntrySize { bytes } => (19, *bytes),
                ElfDynamicDirective::PTLRelocationsMode { mode } => (
                    20,
                    match mode {
                        ElfPLTRelocationsMode::Rel => 17,
                        ElfPLTRelocationsMode::Rela => 7,
                        ElfPLTRelocationsMode::Unknown(other) => *other,
                    },
                ),
                ElfDynamicDirective::Debug { address } => (21, *address),
                ElfDynamicDirective::RelocationsWillModifyText => (22, 0),
                ElfDynamicDirective::JumpRel { address } => (23, *address),
                ElfDynamicDirective::BindNow => (24, 0),
                ElfDynamicDirective::Flags1(flags) => (0x6ffffffb, Bitfield::write(flags)),
                ElfDynamicDirective::Unknown { tag, value } => (*tag, *value),
            };
            match self.object.env.class {
                ElfClass::Elf32 => {
                    self.write_raw::<i32>(
                        tag.try_into()
                            .map_err(|_| WriteError::DynamicValueDoesNotFit { value: tag })?,
                    )?;
                    self.write_raw::<u32>(
                        value
                            .try_into()
                            .map_err(|_| WriteError::DynamicValueDoesNotFit { value })?,
                    )?;
                }
                ElfClass::Elf64 => {
                    self.write_raw(tag)?;
                    self.write_raw(value)?;
                }
            }
        }

        Ok(())
    }

    fn write_padding(&mut self, part: &Part<I::SectionId>) -> Result<(), WriteError<I>> {
        let metadata = self.layout.metadata(part);
        let len = metadata.file.as_ref().expect("padding must be present in the file").len as usize;
        let padding = vec![0; len];
        self.writer.write_all(&padding)?;
        Ok(())
    }

    fn find_section_names_string_table(&self) -> Result<u16, WriteError<I>> {
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
        self.object.sections.keys().position(|k| k == id).expect("inconsistent section id")
    }

    fn perms_to_section_flags(&self, perms: &ElfPermissions) -> RawSectionHeaderFlags {
        RawSectionHeaderFlags {
            write: perms.write,
            alloc: perms.read,
            exec: perms.execute,
            ..RawSectionHeaderFlags::zero()
        }
    }

    fn raw_type_size<T: RawType>(&self) -> u16 {
        T::size(self.object.env.class) as _
    }

    fn write_raw<T: RawType>(&mut self, value: T) -> Result<(), WriteError<I>> {
        value.write(self.object.env.class, self.object.env.endian, self.writer)?;
        Ok(())
    }
}
