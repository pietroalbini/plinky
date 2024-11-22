use crate::errors::LoadError;
use crate::raw::RawHashHeader;
use crate::reader::sections::{SectionMetadata, SectionReader};
use crate::{ElfHash, ElfSectionContent};

pub(super) fn read(
    reader: &mut SectionReader<'_, '_>,
    meta: &dyn SectionMetadata,
) -> Result<ElfSectionContent, LoadError> {
    let mut cursor = reader.content_cursor()?;

    let hash_header: RawHashHeader = cursor.read_raw()?;
    let mut hash = ElfHash {
        symbol_table: meta.section_link(),
        buckets: Vec::with_capacity(hash_header.bucket_count as _),
        chain: Vec::with_capacity(hash_header.chain_count as _),
    };
    for _ in 0..hash_header.bucket_count {
        hash.buckets.push(cursor.read_raw()?);
    }
    for _ in 0..hash_header.chain_count {
        hash.chain.push(cursor.read_raw()?);
    }
    Ok(ElfSectionContent::Hash(hash))
}
