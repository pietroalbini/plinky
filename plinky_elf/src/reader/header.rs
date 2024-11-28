use crate::errors::LoadError;
use crate::ids::ElfSectionId;
use crate::raw::{RawHeader, RawIdentification, RawSectionHeader};
use crate::reader::ReadCursor;
use crate::reader::program_header::read_program_header;
use crate::{ElfABI, ElfClass, ElfEndian, ElfEnvironment, ElfMachine, ElfSegment, ElfType};
use std::collections::BTreeMap;
use std::num::NonZeroU64;

pub(crate) fn read_header(cursor: &mut ReadCursor<'_>) -> Result<ReadHeader, LoadError> {
    let identification: RawIdentification = cursor.read_raw()?;
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

    cursor.class = class;
    cursor.endian = endian;
    let header: RawHeader = cursor.read_raw()?;
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

    let mut sections = BTreeMap::new();
    if header.section_headers_offset != 0 {
        for idx in 0..header.section_header_count {
            cursor.seek_to(
                header.section_headers_offset + (header.section_header_size as u64 * idx as u64),
            )?;
            let raw: RawSectionHeader = cursor.read_raw().map_err(|e| {
                LoadError::FailedToParseSectionHeader { idx: idx.into(), inner: Box::new(e) }
            })?;
            sections.insert(ElfSectionId { index: idx.into() }, raw);
        }
    }

    let mut segments = Vec::new();
    if header.program_headers_offset != 0 {
        for idx in 0..header.program_header_count {
            cursor.seek_to(
                header.program_headers_offset + (header.program_header_size as u64 * idx as u64),
            )?;
            segments.push(read_program_header(cursor)?);
        }
    }

    Ok(ReadHeader {
        env: ElfEnvironment { class, endian, abi, machine },
        type_,
        entry: NonZeroU64::new(header.entry),
        sections,
        segments,
        section_names_table: ElfSectionId { index: header.section_names_table_index.into() },
    })
}

pub(super) struct ReadHeader {
    pub(super) env: ElfEnvironment,
    pub(super) type_: ElfType,
    pub(super) entry: Option<NonZeroU64>,
    pub(super) segments: Vec<ElfSegment>,
    pub(super) sections: BTreeMap<ElfSectionId, RawSectionHeader>,
    pub(super) section_names_table: ElfSectionId,
}
