use crate::raw::RawGnuHashHeader;
use crate::reader::sections::{SectionMetadata, SectionReader};
use crate::{ElfClass, ElfGnuHash, LoadError};

pub(super) fn read(
    reader: &mut SectionReader<'_, '_>,
    meta: &dyn SectionMetadata,
) -> Result<ElfGnuHash, LoadError> {
    let mut cursor = reader.content_cursor()?;

    let gnu_hash_header: RawGnuHashHeader = cursor.read_raw()?;
    let mut gnu_hash = ElfGnuHash {
        symbol_table: meta.section_link(),
        bloom_shift: gnu_hash_header.bloom_shift,
        symbols_offset: gnu_hash_header.symbols_offset,
        bloom: Vec::with_capacity(gnu_hash_header.bloom_count as _),
        buckets: Vec::with_capacity(gnu_hash_header.buckets_count as _),
        chain: Vec::new(),
    };

    for _ in 0..gnu_hash_header.bloom_count {
        gnu_hash.bloom.push(match reader.parent_cursor.class {
            ElfClass::Elf32 => cursor.read_raw::<u32>()?.into(),
            ElfClass::Elf64 => cursor.read_raw::<u64>()?,
        });
    }

    for _ in 0..gnu_hash_header.buckets_count {
        gnu_hash.buckets.push(cursor.read_raw()?);
    }

    // This is == and not <= to ensure we error with an EOF. The cursor is restricted to the
    // current section anyway, so there is no risk of reading out of bounds.
    while cursor.current_position()? != reader.content_len {
        gnu_hash.chain.push(cursor.read_raw()?);
    }

    Ok(gnu_hash)
}
