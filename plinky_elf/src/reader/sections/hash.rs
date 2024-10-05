use crate::errors::LoadError;
use crate::ids::ElfSectionId;
use crate::raw::{RawHashHeader, RawSectionHeader};
use crate::reader::ReadCursor;
use crate::{ElfHash, ElfSectionContent};

pub(super) fn read(
    header: &RawSectionHeader,
    raw_content: &[u8],
    cursor: &mut ReadCursor,
) -> Result<ElfSectionContent, LoadError> {
    let mut inner = std::io::Cursor::new(raw_content);
    let mut cursor = cursor.duplicate(&mut inner);

    let hash_header: RawHashHeader = cursor.read_raw()?;
    let mut hash = ElfHash {
        symbol_table: ElfSectionId { index: header.link },
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
