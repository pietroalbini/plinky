mod dynamic;
mod group;
mod hash;
mod notes;
mod program;
mod reader;
mod relocations_table;
mod string_table;
mod symbol_table;
mod uninit;
mod unknown;

use crate::errors::LoadError;
use crate::ids::{ElfSectionId, ElfStringId};
use crate::raw::RawSectionHeader;
use crate::reader::sections::reader::{HeaderMetadata, SectionMetadata, SectionReader};
use crate::reader::ReadCursor;
use crate::{ElfDeduplication, ElfSection, ElfSectionContent};

pub(super) fn read_section(
    cursor: &mut ReadCursor<'_>,
    section_names_table: ElfSectionId,
    current_section: ElfSectionId,
    header: RawSectionHeader,
) -> Result<ElfSection, LoadError> {
    read_section_inner(cursor, section_names_table, current_section, header).map_err(|inner| {
        LoadError::FailedToParseSection { idx: current_section.index as _, inner: Box::new(inner) }
    })
}

fn read_section_inner(
    cursor: &mut ReadCursor<'_>,
    section_names_table: ElfSectionId,
    current_section: ElfSectionId,
    header: RawSectionHeader,
) -> Result<ElfSection, LoadError> {
    let ty = match header.type_ {
        0 => SectionType::Null,
        1 => SectionType::Program,
        2 => SectionType::SymbolTable { dynsym: false },
        3 => SectionType::StringTable,
        4 => SectionType::Rela,
        5 => SectionType::Hash,
        6 => SectionType::Dynamic,
        7 => SectionType::Note,
        8 => SectionType::Uninit,
        9 => SectionType::Rel,
        11 => SectionType::SymbolTable { dynsym: true },
        17 => SectionType::Group,
        other => SectionType::Unknown(other),
    };

    // The info link flag is used to indicate the info field contains a link to a section table,
    // which only makes sense for relocations. The flag doesn't actually seem to be required
    // though, as for example GCC emits it while NASM doesn't. To catch unknown uses of the flag,
    // we error out if the flag is set for a non-relocation section.
    if header.flags.info_link && !matches!(ty, SectionType::Rel | SectionType::Rela) {
        return Err(LoadError::UnsupportedInfoLinkFlag(current_section.index));
    }

    if header.flags.strings {
        // The spec says the entries_size field determines how long each char is, but there is no
        // point implementing support for this unless an actual object needs it. Error out for now
        // if this happens, to avoid malformed programs being emitted.
        if header.entries_size != 1 {
            return Err(LoadError::UnsupportedStringsWithSizeNotOne {
                section_idx: current_section.index,
                size: header.entries_size,
            });
        }
        // Not sure if there is any valid use of SHF_STRINGS outside of SHF_MERGE or it being
        // redundantly applied to string tables. Error out for now, if a valid use is found the
        // linker will need to be updated to handle it.
        if !(header.flags.merge || matches!(ty, SectionType::StringTable)) {
            return Err(LoadError::UnexpectedStringsFlag { section_idx: current_section.index });
        }
    }

    let mut reader = SectionReader {
        parent_cursor: cursor,
        content_len: header.size,
        content_start: header.offset,
    };
    let meta = HeaderMetadata::new(&header, current_section);

    // Ensure the deduplication flags are only applied to program sections.
    match (ty, meta.deduplication_flag()) {
        (SectionType::Program, _) => {}
        (_, Err(_) | Ok(ElfDeduplication::Disabled)) => {}
        _ => {
            return Err(LoadError::MergeFlagOnUnsupportedSection {
                section_idx: current_section.index,
            });
        }
    }

    let content = match ty {
        SectionType::Null => ElfSectionContent::Null,
        SectionType::Program => program::read(&mut reader, &meta)?,
        SectionType::SymbolTable { dynsym } => symbol_table::read(&mut reader, &meta, dynsym)?,
        SectionType::StringTable => string_table::read(&mut reader)?,
        SectionType::Rel => relocations_table::read_rel(&mut reader, &meta)?,
        SectionType::Rela => relocations_table::read_rela(&mut reader, &meta)?,
        SectionType::Note => notes::read(&mut reader)?,
        SectionType::Uninit => uninit::read(&mut reader, &meta)?,
        SectionType::Group => group::read(&mut reader, &meta)?,
        SectionType::Hash => hash::read(&mut reader, &meta)?,
        SectionType::Dynamic => dynamic::read(&mut reader, &meta)?,
        SectionType::Unknown(other) => unknown::read(&mut reader, other)?,
    };

    Ok(ElfSection {
        name: ElfStringId { section: section_names_table, offset: header.name_offset },
        memory_address: header.memory_address,
        part_of_group: header.flags.group,
        content,
    })
}

#[derive(Clone, Copy)]
enum SectionType {
    Null,
    Program,
    SymbolTable { dynsym: bool },
    StringTable,
    Rel,
    Rela,
    Note,
    Uninit,
    Group,
    Hash,
    Dynamic,
    Unknown(u32),
}
