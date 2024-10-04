#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ElfSectionId {
    pub index: u32,
}

impl std::fmt::Debug for ElfSectionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "section#{}", self.index)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ElfSymbolId {
    pub section: ElfSectionId,
    pub index: u32,
}

impl std::fmt::Debug for ElfSymbolId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "symbol#{}#{}", self.section.index, self.index)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ElfStringId {
    pub section: ElfSectionId,
    pub offset: u32,
}

impl std::fmt::Debug for ElfStringId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "string#{}#{}", self.section.index, self.offset)
    }
}
