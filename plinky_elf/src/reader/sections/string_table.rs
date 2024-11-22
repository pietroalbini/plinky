use crate::errors::LoadError;
use crate::{ElfSectionContent, ElfStringTable};
use std::collections::BTreeMap;
use crate::reader::sections::SectionReader;

pub(super) fn read(reader: &mut SectionReader<'_, '_>) -> Result<ElfSectionContent, LoadError> {
    let raw_content = reader.content()?;

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
    Ok(ElfSectionContent::StringTable(ElfStringTable::new(strings)))
}
