use crate::repr::symbols::Symbol;
use plinky_elf::ids::ElfSectionId;
use plinky_elf::{ElfHash, ElfSectionContent};

pub(super) fn create_sysv_hash<'a>(
    symbols: impl Iterator<Item = &'a Symbol>,
    symbol_table: ElfSectionId,
) -> ElfSectionContent {
    let names = symbols.map(|sym| sym.name()).collect::<Vec<_>>();

    let mut buckets = vec![0; num_buckets(names.len())];
    let mut chain = vec![0; names.len()];

    // The hash table is structured as a list of buckets, and the "chains" list.
    //
    // The list of bucket has a dynamic size (chose by the num_buckets function), and each element
    // in the list of buckets contains the offset of an element in the chains list.
    //
    // The chains list has the same size as the number of symbols we generate the hash for. The
    // chain slot at a given offset corresponds to the symbol in the given offset of the symbol
    // table. The value in each slot of the chains list is an offset to another element in the
    // chains list, or 0 if lookup should be stopped.
    //
    // This means that with a bucket list of [0, 1] and a chain list of [0, 2, 3, 0], when
    // processing the hash table looking for a symbol in the first bucket, we do:
    //
    // - Look up symbol at offset 1.
    // - If it's not the symbol we want, load offset 1 in the chain.
    // - Look up symbol at offset 2, due to the contents of the chain.
    // - If it's not the symbol we want, load offset 2 in the chain.
    // - Look up symbol at offset 3, due to the contents of the chain.
    // - If it's not the symbol we want, load offset 3 in the chain.
    // - Stop, because the content of the chain is zero.
    //
    // To generate the table, we iterate over the list of symbols, and whenever a symbol needs to
    // fit in the bucket we move the content of the bucket to the slot in the chain, and update the
    // bucket to point to the new chain slot.
    for (pos, name) in names.into_iter().enumerate() {
        let hash = elf_hash(name.resolve().as_bytes());
        let bucket = hash as usize % buckets.len();

        let existing_value_in_bucket = buckets[bucket];
        buckets[bucket] = pos as u32;
        chain[pos] = existing_value_in_bucket;
    }

    ElfSectionContent::Hash(ElfHash { symbol_table, buckets, chain })
}

pub(crate) fn num_buckets(symbols_count: usize) -> usize {
    // Different linkers have a way to choose the number of buckets:
    //
    // - Gold choses between a predefined set of bucket sizes:
    //   https://github.com/bminor/binutils-gdb/blob/89bd22ef5b7fec9ac9e760de276781b181f58ee0/gold/dynobj.cc#L862-L873
    //
    // - LLD creates as many buckets as there are symbols:
    //   https://github.com/llvm/llvm-project/blob/8917afaf0ec2ebe390284e3727e720eaf97967eb/lld/ELF/SyntheticSections.cpp#L2509-L2510
    //
    // - Mold creates as many buckets as there are symbols:
    //   https://github.com/rui314/mold/blob/8cd85aaa29093a315d2b905bdcab379ac922e73a/elf/output-chunks.cc#L1776
    //
    // For plinky we choose the Gold approach here.

    // This list comes from Gold, which comes from the original LD. It should be processed in
    // windows of two, where the first element of the window is the number of buckets and the
    // second element is the maximum number of symbols to use that number of buckets.
    const GOLD_BUCKETS: &[usize] = &[
        1, 3, 17, 37, 67, 97, 131, 197, 263, 521, 1031, 2053, 4099, 8209, 16411, 32771, 65537,
        131101, 262147,
    ];

    for [buckets, elements_num] in GOLD_BUCKETS.array_windows() {
        if *elements_num >= symbols_count {
            return *buckets;
        }
    }
    *GOLD_BUCKETS.last().unwrap()
}

fn elf_hash(data: &[u8]) -> u32 {
    let mut h = 0;
    let mut g;
    for byte in data {
        h = (h << 4) + *byte as u32;
        g = h & 0xf0000000;
        if g != 0 {
            h ^= g >> 24;
        }
        h &= !g;
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_elf_hash() {
        // Test suite from https://flapenguin.me/elf-dt-hash
        assert_eq!(0x00000000, elf_hash(b""));
        assert_eq!(0x077905a6, elf_hash(b"printf"));
        assert_eq!(0x0006cf04, elf_hash(b"exit"));
        assert_eq!(0x0b09985c, elf_hash(b"syscall"));
        assert_eq!(0x03987915, elf_hash(b"flapenguin.me"));
    }
}
