use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub struct ElfStringTable {
    strings: BTreeMap<u32, String>,
}

impl ElfStringTable {
    pub fn new(strings: BTreeMap<u32, String>) -> Self {
        Self { strings }
    }

    pub fn get(&self, offset: u32) -> Option<&str> {
        if let Some(string) = self.strings.get(&offset) {
            // Simple case: the offset starts at the beginning of the string.
            Some(string)
        } else if let Some((&prev_offset, prev_string)) = self.strings.range(..offset).last() {
            // Some compilers perform an optimization when a string is a suffix of another string,
            // and return an offset inside the existing string.
            if (offset - prev_offset) as usize > prev_string.len() {
                // Reached out of bounds.
                None
            } else {
                Some(&prev_string[(offset - prev_offset) as usize..])
            }
        } else {
            None
        }
    }

    pub fn all(&self) -> impl Iterator<Item = &str> {
        self.strings.values().map(|s| s.as_str())
    }

    pub(crate) fn all_with_offsets(&self) -> impl Iterator<Item = (u32, &str)> {
        self.strings.iter().map(|(o, s)| (*o, s.as_str()))
    }

    pub fn len(&self) -> usize {
        self.strings.values().map(|s| s.len() + 1).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get() {
        let mut btree = BTreeMap::new();
        btree.insert(0, "Hello world".into());
        btree.insert(20, "Foo".into());
        btree.insert(30, "Bar".into());
        btree.insert(33, "Baz".into());
        let strings = ElfStringTable::new(btree);

        assert_eq!("Hello world", strings.get(0).unwrap());
        assert_eq!("Foo", strings.get(20).unwrap());
        assert_eq!("Bar", strings.get(30).unwrap());
        assert_eq!("Baz", strings.get(33).unwrap());
        assert_eq!("ar", strings.get(31).unwrap());
        assert_eq!(None, strings.get(50));
    }
}
