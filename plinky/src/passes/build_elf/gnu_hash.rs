use crate::repr::symbols::Symbol;

/// How many symbols on average should be in a hash table bucket.
const LOAD_FACTOR: usize = 4;

pub(crate) struct GnuHasher {
    num_buckets: u32,
}

impl GnuHasher {
    pub(crate) fn new<'a>(symbols: impl Iterator<Item = &'a Symbol>) -> Self {
        let symbols_count = symbols.filter(|s| should_hash_symbol(*s)).count();
        let num_buckets = (symbols_count / LOAD_FACTOR + 1).try_into().expect("too many symbols");

        Self { num_buckets }
    }

    pub(crate) fn hash(&self, symbol: &Symbol) -> GnuHashResult {
        if should_hash_symbol(symbol) {
            let hash = gnu_hash(symbol.name().resolve().as_bytes());
            GnuHashResult::Hashed { bucket: hash % self.num_buckets }
        } else {
            GnuHashResult::NotHashed
        }
    }
}

pub(crate) enum GnuHashResult {
    Hashed { bucket: u32 },
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
