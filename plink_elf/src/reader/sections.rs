use crate::errors::LoadError;
use crate::reader::Cursor;
use crate::{
    NoteSection, ProgramSection, RawBytes, Section, SectionContent, StringTable, Symbol,
    SymbolBinding, SymbolDefinition, SymbolTable, SymbolType, UnknownSection,
};
use std::cell::RefCell;
use std::collections::BTreeMap;

pub(super) fn read_sections(
    cursor: &mut Cursor<'_>,
    offset: u64,
    count: u16,
    size: u16,
    section_names_table_index: u16,
) -> Result<Vec<Section>, LoadError> {
    if offset == 0 {
        return Ok(Vec::new());
    }

    let mut sections = Vec::new();
    for idx in 0..count {
        cursor.seek_to(offset + (size as u64 * idx as u64))?;
        sections.push(read_section(cursor, section_names_table_index)?);
    }

    let resolve_str = |pending: &RefCell<PendingString>| -> Result<(), LoadError> {
        let mut mutable = pending.borrow_mut();
        if let PendingString::Ref { section, offset } = &mut *mutable {
            match sections.get(*section as usize).map(|s| &s.content) {
                Some(SectionContent::StringTable(table)) => {
                    *mutable = PendingString::Resolved(
                        table
                            .get(*offset)
                            .ok_or(LoadError::MissingString(*section, *offset))?
                            .to_string(),
                    );
                    Ok(())
                }
                Some(_) => Err(LoadError::WrongStringTableType(*section)),
                None => Err(LoadError::MissingStringTable(*section)),
            }
        } else {
            Ok(())
        }
    };
    for section in &sections {
        resolve_str(&section.name)?;
        if let SectionContent::SymbolTable(table) = &section.content {
            for symbol in &table.symbols {
                resolve_str(&symbol.name)?;
            }
        }
    }

    let remove_pending_str = |pending: RefCell<PendingString>| -> String {
        match pending.into_inner() {
            PendingString::Ref { .. } => unreachable!("unresolved string"),
            PendingString::Resolved(inner) => inner,
        }
    };
    Ok(sections
        .into_iter()
        .map(|s| Section {
            name: remove_pending_str(s.name),
            writeable: s.writeable,
            allocated: s.allocated,
            executable: s.executable,
            memory_address: s.memory_address,
            content: match s.content {
                SectionContent::Null => SectionContent::Null,
                SectionContent::Program(p) => SectionContent::Program(p),
                SectionContent::SymbolTable(s) => SectionContent::SymbolTable(SymbolTable {
                    symbols: s
                        .symbols
                        .into_iter()
                        .map(|s| Symbol {
                            name: remove_pending_str(s.name),
                            binding: s.binding,
                            type_: s.type_,
                            definition: s.definition,
                            value: s.value,
                            size: s.size,
                        })
                        .collect(),
                }),
                SectionContent::StringTable(s) => SectionContent::StringTable(s),
                SectionContent::Note(n) => SectionContent::Note(n),
                SectionContent::Unknown(u) => SectionContent::Unknown(u),
            },
        })
        .collect())
}

fn read_section(
    cursor: &mut Cursor<'_>,
    section_names_table_index: u16,
) -> Result<Section<RefCell<PendingString>>, LoadError> {
    let name_offset = cursor.read_u32()?;
    let type_ = cursor.read_u32()?;
    let flags = cursor.read_usize()?;
    let memory_address = cursor.read_usize()?;
    let offset = cursor.read_usize()?;
    let size = cursor.read_usize()?;
    let link = cursor.read_u32()?;
    let _info = cursor.read_u32()?;
    let _addr_align = cursor.read_usize()?;
    let _entries_size = cursor.read_usize()?;

    let raw_content = cursor.read_vec_at(offset, size)?;
    let content = match type_ {
        0 => SectionContent::Null,
        1 => SectionContent::Program(ProgramSection {
            raw: RawBytes(raw_content),
        }),
        2 => read_symbol_table(cursor, &raw_content, link as _)?,
        3 => read_string_table(&raw_content)?,
        7 => SectionContent::Note(NoteSection {
            raw: RawBytes(raw_content),
        }),
        other => SectionContent::Unknown(UnknownSection {
            id: other,
            raw: RawBytes(raw_content),
        }),
    };

    Ok(Section {
        name: RefCell::new(PendingString::Ref {
            section: section_names_table_index,
            offset: name_offset,
        }),
        writeable: flags & 0x1 > 0,
        allocated: flags & 0x2 > 0,
        executable: flags & 0x4 > 0,
        memory_address,
        content,
    })
}

fn read_string_table(
    raw_content: &[u8],
) -> Result<SectionContent<RefCell<PendingString>>, LoadError> {
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

fn read_symbol_table(
    cursor: &mut Cursor<'_>,
    raw_content: &[u8],
    strings_table: u16,
) -> Result<SectionContent<RefCell<PendingString>>, LoadError> {
    let mut inner = std::io::Cursor::new(raw_content);
    let mut cursor = cursor.duplicate(&mut inner);

    let mut symbols = Vec::new();
    loop {
        if cursor.current_position()? == raw_content.len() as u64 {
            break;
        }
        symbols.push(read_symbol(&mut cursor, strings_table)?);
    }

    Ok(SectionContent::SymbolTable(SymbolTable { symbols }))
}

fn read_symbol(
    cursor: &mut Cursor<'_>,
    strings_table: u16,
) -> Result<Symbol<RefCell<PendingString>>, LoadError> {
    let name_offset = cursor.read_u32()?;
    let info = cursor.read_u8()?;
    let _ = cursor.read_u8()?; // Reserved
    let definition = cursor.read_u16()?;
    let value = cursor.read_usize()?;
    let size = cursor.read_usize()?;

    Ok(Symbol {
        name: RefCell::new(PendingString::Ref {
            section: strings_table,
            offset: name_offset,
        }),
        binding: match (info & 0b11110000) >> 4 {
            0 => SymbolBinding::Local,
            1 => SymbolBinding::Global,
            2 => SymbolBinding::Weak,
            other => SymbolBinding::Unknown(other),
        },
        type_: match info & 0b1111 {
            0 => SymbolType::NoType,
            1 => SymbolType::Object,
            2 => SymbolType::Function,
            3 => SymbolType::Section,
            4 => SymbolType::File,
            other => SymbolType::Unknown(other),
        },
        definition: match definition {
            0x0000 => SymbolDefinition::Undefined, // SHN_UNDEF
            0xFFF1 => SymbolDefinition::Absolute,  // SHN_ABS
            0xFFF2 => SymbolDefinition::Common,    // SHN_COMMON
            other => SymbolDefinition::Section(other),
        },
        value,
        size,
    })
}

#[derive(Debug)]
enum PendingString {
    Ref { section: u16, offset: u32 },
    Resolved(String),
}
