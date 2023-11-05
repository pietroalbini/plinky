use crate::errors::LoadError;
use crate::reader::program_header::{read_program_header, SegmentContentMapping};
use crate::reader::sections::read_sections;
use crate::reader::{Cursor, PendingIds, PendingSectionId};
use crate::{ElfABI, ElfClass, ElfEndian, ElfEnvironment, ElfMachine, ElfObject, ElfType};
use std::collections::BTreeMap;
use std::num::NonZeroU64;

pub(crate) fn read_object(cursor: &mut Cursor<'_>) -> Result<ElfObject<PendingIds>, LoadError> {
    read_magic(cursor)?;
    let class = read_class(cursor)?;
    let endian = read_endian(cursor)?;

    // Use the provided endianness for the rest of the reading.
    cursor.class = Some(class);
    cursor.endian = Some(endian);

    read_version_u8(cursor)?;
    let abi = read_abi(cursor)?;
    read_abi_version(cursor, abi)?;
    cursor.skip_padding::<7>()?;
    let type_ = read_type(cursor)?;
    let machine = read_machine(cursor)?;
    read_version_u32(cursor)?;
    let entry = cursor.read_usize()?;

    let mut segment_content_map: SegmentContentMapping = BTreeMap::new();

    let program_headers_offset = cursor.read_usize()?;
    let section_headers_offset = cursor.read_usize()?;

    let flags = cursor.read_u32()?;

    let _elf_header_size = cursor.read_u16()?;
    let program_header_size = cursor.read_u16()?;
    let program_header_count = cursor.read_u16()?;
    let section_header_size = cursor.read_u16()?;
    let section_header_count = cursor.read_u16()?;
    let section_names_table_index = cursor.read_u16()?;

    let sections = read_sections(
        cursor,
        &mut segment_content_map,
        section_headers_offset,
        section_header_count,
        section_header_size,
        PendingSectionId(section_names_table_index as _),
    )?;

    let mut segments = Vec::new();
    if program_headers_offset != 0 {
        for idx in 0..program_header_count {
            cursor.seek_to(program_headers_offset + (program_header_size as u64 * idx as u64))?;
            segments.push(read_program_header(cursor, &segment_content_map)?);
        }
    }

    Ok(ElfObject::<PendingIds> {
        env: ElfEnvironment {
            class,
            endian,
            abi,
            machine,
        },
        type_,
        entry: NonZeroU64::new(entry),
        flags,
        sections,
        segments,
    })
}

fn read_magic(cursor: &mut Cursor<'_>) -> Result<(), LoadError> {
    let magic = cursor.read_bytes()?;
    if magic == [0x7F, b'E', b'L', b'F'] {
        Ok(())
    } else {
        Err(LoadError::BadMagic(magic))
    }
}

fn read_class(cursor: &mut Cursor<'_>) -> Result<ElfClass, LoadError> {
    match cursor.read_u8()? {
        1 => Ok(ElfClass::Elf32),
        2 => Ok(ElfClass::Elf64),
        other => Err(LoadError::BadClass(other)),
    }
}

fn read_endian(cursor: &mut Cursor<'_>) -> Result<ElfEndian, LoadError> {
    match cursor.read_u8()? {
        1 => Ok(ElfEndian::Little),
        2 => Ok(ElfEndian::Big),
        other => Err(LoadError::BadEndian(other)),
    }
}

fn read_version_u8(cursor: &mut Cursor<'_>) -> Result<(), LoadError> {
    match cursor.read_u8()? {
        1 => Ok(()),
        other => Err(LoadError::BadVersion(other as _)),
    }
}

fn read_version_u32(cursor: &mut Cursor<'_>) -> Result<(), LoadError> {
    match cursor.read_u32()? {
        1 => Ok(()),
        other => Err(LoadError::BadVersion(other)),
    }
}

fn read_abi(cursor: &mut Cursor<'_>) -> Result<ElfABI, LoadError> {
    match cursor.read_u8()? {
        0 => Ok(ElfABI::SystemV),
        other => Err(LoadError::BadAbi(other)),
    }
}

fn read_abi_version(cursor: &mut Cursor<'_>, abi: ElfABI) -> Result<(), LoadError> {
    let version = cursor.read_u8()?;
    match abi {
        ElfABI::SystemV => match version {
            0 => Ok(()),
            other => Err(LoadError::BadAbiVersion(abi, other)),
        },
    }
}

fn read_type(cursor: &mut Cursor<'_>) -> Result<ElfType, LoadError> {
    match cursor.read_u16()? {
        1 => Ok(ElfType::Relocatable),
        2 => Ok(ElfType::Executable),
        3 => Ok(ElfType::SharedObject),
        4 => Ok(ElfType::Core),
        other => Err(LoadError::BadType(other)),
    }
}

fn read_machine(cursor: &mut Cursor<'_>) -> Result<ElfMachine, LoadError> {
    match cursor.read_u16()? {
        3 => Ok(ElfMachine::X86),
        62 => Ok(ElfMachine::X86_64),
        other => Err(LoadError::BadMachine(other)),
    }
}
