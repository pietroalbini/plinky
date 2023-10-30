use crate::errors::LoadError;
use crate::utils::ReadSeek;
use crate::{
    Class, Endian, Machine, Object, RawBytes, Section, SectionContent, Segment, SegmentContent,
    StringTable, Type, ABI,
};
use std::collections::BTreeMap;
use std::io::SeekFrom;
use std::num::NonZeroU64;

pub(crate) struct ObjectReader<'a> {
    reader: &'a mut dyn ReadSeek,
    class: Option<Class>,
    endian: Option<Endian>,
}

impl<'a> ObjectReader<'a> {
    pub(crate) fn new(reader: &'a mut dyn ReadSeek) -> Self {
        Self {
            reader,
            class: None,
            endian: None,
        }
    }

    pub(crate) fn read(mut self) -> Result<Object, LoadError> {
        self.read_magic()?;
        let class = self.read_class()?;
        let endian = self.read_endian()?;

        // Use the provided endianness for the rest of the reading.
        self.class = Some(class);
        self.endian = Some(endian);

        self.read_version_u8()?;
        let abi = self.read_abi()?;
        self.read_abi_version(abi)?;
        self.skip_padding::<7>()?;
        let type_ = self.read_type()?;
        let machine = self.read_machine()?;
        self.read_version_u32()?;
        let entry = self.read_usize()?;

        let program_headers_offset = self.read_usize()?;
        let section_headers_offset = self.read_usize()?;

        let flags = self.read_u32()?;

        let _elf_header_size = self.read_u16()?;
        let program_header_size = self.read_u16()?;
        let program_header_count = self.read_u16()?;
        let section_header_size = self.read_u16()?;
        let section_header_count = self.read_u16()?;
        let section_names_table_index = self.read_u16()?;

        let sections = self.read_sections(
            section_headers_offset,
            section_header_count,
            section_header_size,
            section_names_table_index,
        )?;

        let mut segments = Vec::new();
        if program_headers_offset != 0 {
            for idx in 0..program_header_count {
                self.reader.seek(SeekFrom::Start(
                    program_headers_offset + (program_header_size as u64 * idx as u64),
                ))?;
                segments.push(self.read_program_header()?);
            }
        }

        Ok(Object {
            class,
            endian,
            abi,
            type_,
            machine,
            entry: NonZeroU64::new(entry),
            flags,
            sections,
            segments,
        })
    }

    fn read_magic(&mut self) -> Result<(), LoadError> {
        let magic = self.read_bytes()?;
        if magic == [0x7F, b'E', b'L', b'F'] {
            Ok(())
        } else {
            Err(LoadError::BadMagic(magic))
        }
    }

    fn read_class(&mut self) -> Result<Class, LoadError> {
        match self.read_u8()? {
            1 => Ok(Class::Elf32),
            2 => Ok(Class::Elf64),
            other => Err(LoadError::BadClass(other)),
        }
    }

    fn read_endian(&mut self) -> Result<Endian, LoadError> {
        match self.read_u8()? {
            1 => Ok(Endian::Little),
            2 => Ok(Endian::Big),
            other => Err(LoadError::BadEndian(other)),
        }
    }

    fn read_version_u8(&mut self) -> Result<(), LoadError> {
        match self.read_u8()? {
            1 => Ok(()),
            other => Err(LoadError::BadVersion(other as _)),
        }
    }

    fn read_version_u32(&mut self) -> Result<(), LoadError> {
        match self.read_u32()? {
            1 => Ok(()),
            other => Err(LoadError::BadVersion(other)),
        }
    }

    fn read_abi(&mut self) -> Result<ABI, LoadError> {
        match self.read_u8()? {
            0 => Ok(ABI::SystemV),
            other => Err(LoadError::BadAbi(other)),
        }
    }

    fn read_abi_version(&mut self, abi: ABI) -> Result<(), LoadError> {
        let version = self.read_u8()?;
        match abi {
            ABI::SystemV => match version {
                0 => Ok(()),
                other => Err(LoadError::BadAbiVersion(abi, other)),
            },
        }
    }

    fn read_type(&mut self) -> Result<Type, LoadError> {
        match self.read_u16()? {
            1 => Ok(Type::Relocatable),
            2 => Ok(Type::Executable),
            3 => Ok(Type::SharedObject),
            4 => Ok(Type::Core),
            other => Err(LoadError::BadType(other)),
        }
    }

    fn read_machine(&mut self) -> Result<Machine, LoadError> {
        match self.read_u16()? {
            3 => Ok(Machine::X86),
            62 => Ok(Machine::X86_64),
            other => Err(LoadError::BadMachine(other)),
        }
    }

    fn read_sections(
        &mut self,
        offset: u64,
        count: u16,
        size: u16,
        section_names_table_index: u16,
    ) -> Result<Vec<Section>, LoadError> {
        if offset == 0 {
            return Ok(Vec::new());
        }

        let mut raw_sections = Vec::new();
        for idx in 0..count {
            self.reader
                .seek(SeekFrom::Start(offset + (size as u64 * idx as u64)))?;
            raw_sections.push(self.read_raw_section()?);
        }

        let names = match raw_sections
            .get(section_names_table_index as usize)
            .map(|s| &s.content)
        {
            Some(SectionContent::StringTable(names)) => names.clone(),
            Some(_) => return Err(LoadError::WrongSectionNamesTableType),
            None => return Err(LoadError::MissingSectionNamesTable),
        };

        let mut sections = Vec::new();
        for raw in raw_sections {
            sections.push(Section {
                name: names
                    .get(raw.name_offset)
                    .map(|s| s.to_string())
                    .ok_or(LoadError::MissingSectionName(raw.name_offset))?,
                writeable: raw.writeable,
                allocated: raw.allocated,
                executable: raw.executable,
                memory_address: raw.memory_address,
                content: raw.content,
            });
        }

        Ok(sections)
    }

    fn read_raw_section(&mut self) -> Result<RawSection, LoadError> {
        let name_offset = self.read_u32()?;
        let type_ = self.read_u32()?;
        let flags = self.read_usize()?;
        let memory_address = self.read_usize()?;
        let offset = self.read_usize()?;
        let size = self.read_usize()?;
        let _link = self.read_u32()?;
        let _info = self.read_u32()?;
        let _addr_align = self.read_usize()?;
        let _entries_size = self.read_usize()?;

        let raw_content = self.read_vec_at(offset, size)?;
        let content = match type_ {
            3 => self.read_string_table(&raw_content)?,
            other => SectionContent::Unknown {
                id: other,
                raw: RawBytes(raw_content),
            },
        };

        Ok(RawSection {
            name_offset,
            writeable: flags & 0x1 > 0,
            allocated: flags & 0x2 > 0,
            executable: flags & 0x4 > 0,
            memory_address,
            content,
        })
    }

    fn read_string_table(&mut self, raw_content: &[u8]) -> Result<SectionContent, LoadError> {
        let mut strings = BTreeMap::new();

        let mut offset: usize = 0;
        while offset < raw_content.len() {
            let terminator = raw_content
                .iter()
                .skip(offset as _)
                .position(|&byte| byte == 0)
                .ok_or(LoadError::UnterminatedString)?;
            strings.insert(
                offset as u32,
                String::from_utf8(raw_content[offset..(offset + terminator)].to_vec())?,
            );

            offset += terminator + 1;
        }

        Ok(SectionContent::StringTable(StringTable::new(strings)))
    }

    fn read_program_header(&mut self) -> Result<Segment, LoadError> {
        // The position of the `flags` field changes depending on whether it's a 32-bit or 64-bit
        // ELF binary.
        let mut _flags = 0;

        let type_ = self.read_u32()?;
        if let Some(Class::Elf64) = &self.class {
            _flags = self.read_u32()?;
        }
        let offset = self.read_usize()?;
        let _virtual_address = self.read_usize()?;
        let _reserved = self.read_usize()?;
        let file_size = self.read_usize()?;
        let _memory_size = self.read_usize()?;
        if let Some(Class::Elf32) = &self.class {
            _flags = self.read_u32()?;
        }
        let _align = self.read_usize()?;
        let contents = self.read_vec_at(offset, file_size)?;

        Ok(Segment {
            content: SegmentContent::Unknown {
                id: type_,
                raw: RawBytes(contents),
            },
        })
    }

    fn read_u8(&mut self) -> Result<u8, LoadError> {
        let bytes = self.read_bytes::<1>()?;
        Ok(bytes[0])
    }

    fn read_u16(&mut self) -> Result<u16, LoadError> {
        let bytes = self.read_bytes()?;
        match self.endian {
            Some(Endian::Big) => Ok(u16::from_be_bytes(bytes)),
            Some(Endian::Little) => Ok(u16::from_le_bytes(bytes)),
            None => panic!("parsing non-u8 without setting endian"),
        }
    }

    fn read_u32(&mut self) -> Result<u32, LoadError> {
        let bytes = self.read_bytes()?;
        match self.endian {
            Some(Endian::Big) => Ok(u32::from_be_bytes(bytes)),
            Some(Endian::Little) => Ok(u32::from_le_bytes(bytes)),
            None => panic!("parsing non-u8 without setting endian"),
        }
    }

    fn read_u64(&mut self) -> Result<u64, LoadError> {
        let bytes = self.read_bytes()?;
        match self.endian {
            Some(Endian::Big) => Ok(u64::from_be_bytes(bytes)),
            Some(Endian::Little) => Ok(u64::from_le_bytes(bytes)),
            None => todo!(),
        }
    }

    fn read_usize(&mut self) -> Result<u64, LoadError> {
        match self.class {
            Some(Class::Elf32) => Ok(self.read_u32()? as _),
            Some(Class::Elf64) => Ok(self.read_u64()?),
            None => panic!("parsing usize without setting class"),
        }
    }

    fn skip_padding<const N: usize>(&mut self) -> Result<(), LoadError> {
        self.read_bytes::<N>()?;
        Ok(())
    }

    fn read_bytes<const N: usize>(&mut self) -> Result<[u8; N], LoadError> {
        let mut buf = [0; N];
        self.reader.read_exact(&mut buf)?;
        Ok(buf)
    }

    fn read_vec_at(&mut self, offset: u64, size: u64) -> Result<Vec<u8>, LoadError> {
        self.reader.seek(SeekFrom::Start(offset))?;
        let mut contents = vec![0; size as _];
        self.reader.read_exact(&mut contents)?;
        Ok(contents)
    }
}

#[derive(Debug)]
struct RawSection {
    name_offset: u32,
    writeable: bool,
    allocated: bool,
    executable: bool,
    memory_address: u64,
    content: SectionContent,
}
