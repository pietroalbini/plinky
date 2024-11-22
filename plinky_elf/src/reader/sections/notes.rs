use crate::errors::LoadError;
use crate::raw::RawNoteHeader;
use crate::reader::sections::SectionReader;
use crate::reader::ReadCursor;
use crate::{
    ElfClass, ElfGnuProperty, ElfNote, ElfNotesTable, ElfUnknownGnuProperty, ElfUnknownNote,
    ElfX86Features2, ElfX86Isa,
};
use plinky_utils::bitfields::Bitfield;
use std::error::Error;

pub(super) fn read(reader: &mut SectionReader<'_, '_>) -> Result<ElfNotesTable, LoadError> {
    let mut cursor = reader.content_cursor()?;

    let mut notes = Vec::new();
    while cursor.current_position()? != reader.content_len as u64 {
        notes.push(read_note(reader, &mut cursor)?);
    }

    Ok(ElfNotesTable { notes })
}

fn read_note(
    reader: &mut SectionReader<'_, '_>,
    cursor: &mut ReadCursor<'_>,
) -> Result<ElfNote, LoadError> {
    let header: RawNoteHeader = cursor.read_raw()?;

    let mut name_bytes = cursor.read_vec(header.name_size as _)?;
    name_bytes.pop(); // Zero-terminated string
    cursor.align_with_padding(4)?;

    let value = cursor.read_vec(header.value_size as _)?;
    cursor.align_with_padding(4)?;

    match (name_bytes.as_slice(), header.type_) {
        (b"GNU", 5) => read_gnu_property(reader, value),
        _ => Ok(ElfNote::Unknown(ElfUnknownNote {
            name: String::from_utf8(name_bytes)?,
            value,
            type_: header.type_,
        })),
    }
}

fn read_gnu_property(
    reader: &mut SectionReader<'_, '_>,
    raw: Vec<u8>,
) -> Result<ElfNote, LoadError> {
    let mut cursor = reader.cursor_for(raw);
    let mut properties = Vec::new();

    'reader: loop {
        let type_: u32 = match cursor.read_raw() {
            Ok(type_) => type_,
            Err(err) => {
                let mut source = err.source();
                while let Some(err) = source.take() {
                    if let Some(err) = err.downcast_ref::<std::io::Error>() {
                        if err.kind() == std::io::ErrorKind::UnexpectedEof {
                            break 'reader;
                        }
                    }
                    source = err.source();
                }
                return Err(err);
            }
        };
        let data_len: u32 = cursor.read_raw()?;
        let data = cursor.read_vec(data_len.into())?;
        cursor.align_with_padding(match cursor.class {
            ElfClass::Elf32 => 4,
            ElfClass::Elf64 => 8,
        })?;

        match type_ {
            // GNU_PROPERTY_X86_FEATURE_2_USED
            0xc0010001 => {
                let mut cursor = reader.cursor_for(data);
                properties.push(ElfGnuProperty::X86Features2Used(
                    ElfX86Features2::read(cursor.read_raw()?)
                        .map_err(LoadError::X86Features2Used)?,
                ));
            }
            // GNU_PROPERTY_X86_ISA_USED
            0xc0010002 => {
                let mut cursor = reader.cursor_for(data);
                properties.push(ElfGnuProperty::X86IsaUsed(
                    ElfX86Isa::read(cursor.read_raw()?).map_err(LoadError::X86IsaUsed)?,
                ));
            }
            _ => {
                properties.push(ElfGnuProperty::Unknown(ElfUnknownGnuProperty { type_, data }));
            }
        }
    }

    Ok(ElfNote::GnuProperties(properties))
}
