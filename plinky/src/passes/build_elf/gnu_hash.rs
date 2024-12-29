use crate::repr::symbols::Symbol;
use plinky_elf::ids::ElfSectionId;
use plinky_elf::{ElfClass, ElfGnuHash, ElfSectionContent};

/// How many symbols on average should be in a hash table bucket.
const LOAD_FACTOR: usize = 4;

/// How many bits to add to the bloom filter for each symbol.
const BLOOM_BITS_PER_SYMBOL: usize = 12;

/// How many bits will the bloom be shifted by.
const BLOOM_SHIFT: usize = 10;

/// Bitmask to apply to each hash stored in the chain.
const MASK_CHAIN_HASH: u32 = 0xfffffffe;

/// Default value for a bucket if there is no symbol in it.
///
/// Unfortunately there is not a standard value to set for the empty bucket. In theory, the index
/// of any symbol would work, as that would just walk through the chain without finding anything.
///
/// We need to keep in mind though that the bucket array contains the symbol index of the first
/// symbol in the bucket's chain (which counts excluded symbols), while the chain doesn't include
/// excluded symbols.
///
/// Knowing that, putting '0' as the default would point to no valid bucket, as symbol 0 is always
/// excluded. All the hash implementations I saw handle this properly, so let's default to 0.
const BUCKET_EMPTY: u32 = 0;

pub(super) fn create_gnu_hash<'a>(
    symbols: impl Iterator<Item = &'a Symbol>,
    symbol_table: ElfSectionId,
    class: ElfClass,
) -> ElfSectionContent {
    let symbols = symbols.collect::<Vec<_>>();
    let hasher = GnuHasher::new(class, symbols.iter().map(|s| *s));

    let mut bloom = vec![0; hasher.bloom_bytes_count as usize];
    let mut symbols_offset = 0;
    let mut buckets = vec![BUCKET_EMPTY; hasher.buckets_count as usize];
    let mut chain = Vec::new();
    let mut state = State::NotHashing;

    for (idx, symbol) in symbols.into_iter().enumerate() {
        match (hasher.hash(symbol), state) {
            (GnuHashResult::Hashed { bucket, hash, bloom_mask, bloom_word }, State::NotHashing) => {
                bloom[bloom_word] |= bloom_mask;
                // The mask sets the least significant bit to zero, which will then be overridden
                // to 1 by the next iteration if this is the last item with this bucket.
                chain.push(hash & MASK_CHAIN_HASH);

                buckets[bucket as usize] = idx as u32;
                state = State::Hashing { current_bucket: bucket };
            }
            (
                GnuHashResult::Hashed { bucket, hash, bloom_word, bloom_mask },
                State::Hashing { current_bucket },
            ) => {
                if bucket != current_bucket {
                    // Set the least significant bit of the previous chain item to 1 to indicate
                    // the previous chain ended.
                    *chain.last_mut().expect("empty chain") |= 1;

                    state = State::Hashing { current_bucket: bucket };
                    if buckets[bucket as usize] == BUCKET_EMPTY {
                        buckets[bucket as usize] = idx as u32;
                    } else {
                        // This likely happens if the symbols list is not sorted by bucket.
                        panic!("inserting a bucket multiple times");
                    }
                }

                bloom[bloom_word] |= bloom_mask;
                // The mask sets the least significant bit to zero, which will then be overridden
                // to 1 by the next iteration if this is the last item with this bucket.
                chain.push(hash & MASK_CHAIN_HASH);
            }

            (GnuHashResult::NotHashed, State::NotHashing) => symbols_offset += 1,
            (GnuHashResult::NotHashed, State::Hashing { .. }) => {
                panic!("found an un-hashable symbol during hashing, is the symbols list sorted?");
            }
        }
    }

    // Ensure the chain is terminated by setting the least significant bit of the last element to 1.
    *chain.last_mut().expect("empty chain") |= 1;

    ElfSectionContent::GnuHash(ElfGnuHash {
        symbol_table,
        bloom_shift: BLOOM_SHIFT as _,
        symbols_offset,
        bloom,
        buckets,
        chain,
    })
}

pub(crate) struct GnuHasher {
    pub(crate) symbols_count: usize,
    pub(crate) buckets_count: u32,
    pub(crate) bloom_bytes_count: u32,
    bloom_entry_size: usize,
}

impl GnuHasher {
    pub(crate) fn new<'a>(class: ElfClass, symbols: impl Iterator<Item = &'a Symbol>) -> Self {
        let symbols_count = symbols.filter(|s| should_hash_symbol(*s)).count();
        let buckets_count = (symbols_count / LOAD_FACTOR + 1).try_into().expect("too many symbols");

        let bloom_entry_size = match class {
            ElfClass::Elf32 => 32,
            ElfClass::Elf64 => 64,
        };
        let bloom_bytes_count =
            u32::try_from((symbols_count * BLOOM_BITS_PER_SYMBOL) / bloom_entry_size + 1)
                .expect("too many symbols")
                // This is an assert in glibc...
                .next_power_of_two();

        Self { symbols_count, buckets_count, bloom_bytes_count, bloom_entry_size }
    }

    pub(crate) fn hash(&self, symbol: &Symbol) -> GnuHashResult {
        if should_hash_symbol(symbol) {
            let hash = gnu_hash(symbol.name().resolve().as_bytes());

            let mut bloom_mask = 0;
            bloom_mask |= 1 << (hash % self.bloom_entry_size as u32);
            bloom_mask |= 1 << ((hash >> BLOOM_SHIFT) % self.bloom_entry_size as u32);

            GnuHashResult::Hashed {
                bucket: hash % self.buckets_count,
                bloom_word: (hash as usize / self.bloom_entry_size)
                    % self.bloom_bytes_count as usize,
                bloom_mask,
                hash,
            }
        } else {
            GnuHashResult::NotHashed
        }
    }
}

#[derive(Debug)]
pub(crate) enum GnuHashResult {
    Hashed { bucket: u32, hash: u32, bloom_word: usize, bloom_mask: u64 },
    NotHashed,
}

fn gnu_hash(data: &[u8]) -> u32 {
    let mut h: u32 = 5381;
    for byte in data {
        h = h.wrapping_mul(33).wrapping_add(*byte as u32);
    }
    h
}

// The GNU hash table supports skipping the hashing of symbols that don't need to be looked up.
fn should_hash_symbol(symbol: &Symbol) -> bool {
    !symbol.is_null_symbol()
}

#[derive(Debug, Clone, Copy)]
enum State {
    NotHashing,
    Hashing { current_bucket: u32 },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gnu_hash() {
        // Test suite from https://flapenguin.me/elf-dt-gnu-hash
        assert_eq!(0x00001505, gnu_hash(b""));
        assert_eq!(0x156b2bb8, gnu_hash(b"printf"));
        assert_eq!(0x7c967e3f, gnu_hash(b"exit"));
        assert_eq!(0xbac212a0, gnu_hash(b"syscall"));
        assert_eq!(0x8ae9f18e, gnu_hash(b"flapenguin.me"));
    }
}
