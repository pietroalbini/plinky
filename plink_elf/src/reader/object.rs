use crate::errors::LoadError;
use crate::raw::{RawIdentification, RawType, RawHeader};
use crate::reader::program_header::{read_program_header, SegmentContentMapping};
use crate::reader::sections::read_sections;
use crate::reader::{PendingIds, PendingSectionId, ReadCursor};
use crate::{ElfABI, ElfClass, ElfEndian, ElfEnvironment, ElfMachine, ElfObject, ElfType};
use std::collections::BTreeMap;
use std::num::NonZeroU64;

pub(crate) fn read_object(cursor: &mut ReadCursor<'_>) -> Result<ElfObject<PendingIds>, LoadError> {
    let identification = RawIdentification::read(cursor)?;
    if identification.magic != [0x7F, b'E', b'L', b'F'] {
        return Err(LoadError::BadMagic(identification.magic));
    }
    if identification.version != 1 {
        return Err(LoadError::BadVersion(identification.version as _));
    }

    let class = match identification.class {
        1 => ElfClass::Elf32,
        2 => ElfClass::Elf64,
        other => return Err(LoadError::BadClass(other)),
    };
    let endian = match identification.endian {
        1 => ElfEndian::Little,
        other => return Err(LoadError::BadEndian(other)),
    };
    let abi = match (identification.abi, identification.abi_version) {
        (0, 0) => ElfABI::SystemV,
        (0, version) => return Err(LoadError::BadAbiVersion(ElfABI::SystemV, version)),
        (abi, _) => return Err(LoadError::BadAbi(abi)),
    };

    cursor.class = Some(class);
    let header = RawHeader::read(cursor)?;
    if header.version != 1 {
        return Err(LoadError::BadVersion(header.version));
    }

    let type_ = match header.type_ {
        1 => ElfType::Relocatable,
        2 => ElfType::Executable,
        3 => ElfType::SharedObject,
        4 => ElfType::Core,
        other => return Err(LoadError::BadType(other)),
    };
    let machine = match header.machine {
        3 => ElfMachine::X86,
        62 => ElfMachine::X86_64,
        other => return Err(LoadError::BadMachine(other)),
    };

    let mut segment_content_map: SegmentContentMapping = BTreeMap::new();

    let sections = read_sections(
        cursor,
        &mut segment_content_map,
        header.section_headers_offset,
        header.section_header_count,
        header.section_header_size,
        PendingSectionId(header.section_names_table_index as _),
    )?;

    let mut segments = Vec::new();
    if header.program_headers_offset != 0 {
        for idx in 0..header.program_header_count {
            cursor.seek_to(header.program_headers_offset + (header.program_header_size as u64 * idx as u64))?;
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
        entry: NonZeroU64::new(header.entry),
        flags: header.flags,
        sections,
        segments,
    })
}
