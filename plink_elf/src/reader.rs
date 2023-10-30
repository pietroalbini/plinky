use crate::errors::LoadError;
use crate::utils::ReadSeek;
use crate::{Class, Endian, Machine, Object, RawBytes, Segment, SegmentType, Type, ABI};
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
        let _section_headers_offset = self.read_usize()?;

        let flags = self.read_u32()?;

        let _elf_header_size = self.read_u16()?;
        let program_header_size = self.read_u16()?;
        let program_header_count = self.read_u16()?;
        let _section_header_size = self.read_u16()?;
        let _section_header_count = self.read_u16()?;
        let _string_table_index = self.read_u16()?;

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

    fn read_program_header(&mut self) -> Result<Segment, LoadError> {
        let type_ = self.read_segment_type()?;
        let flags = self.read_u32()?;
        let _offset = self.read_usize()?;
        let _virtual_address = self.read_usize()?;
        let _reserved = self.read_usize()?;
        let file_size = self.read_usize()?;
        let _memory_size = self.read_usize()?;
        let _align = self.read_usize()?;

        let mut contents = vec![0; file_size as _];
        self.reader.read_exact(&mut contents)?;

        dbg!(type_);

        Ok(Segment {
            type_,
            flags,
            contents: RawBytes(contents),
        })
    }

    fn read_segment_type(&mut self) -> Result<SegmentType, LoadError> {
        match self.read_u32()? {
            0 => Ok(SegmentType::Null),
            1 => Ok(SegmentType::Loadable),
            2 => Ok(SegmentType::DynamicLinkingTables),
            3 => Ok(SegmentType::ProgramInterpreter),
            4 => Ok(SegmentType::Note),
            6 => Ok(SegmentType::ProgramHeaderTable),
            other => Ok(SegmentType::Unknown(other)),
        }
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
}
