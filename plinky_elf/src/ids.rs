use std::fmt::Debug;
use std::hash::Hash;

pub trait ElfIds: Debug + Sized {
    type SectionId: Debug + Clone + Hash + PartialEq + Eq + PartialOrd + Ord + ReprIdGetters;
    type SymbolId: Debug + Clone + Hash + PartialEq + Eq + PartialOrd + Ord + ReprIdGetters;
    type StringId: Debug + Clone + Hash + PartialEq + Eq + PartialOrd + Ord + StringIdGetters<Self>;
}

pub trait StringIdGetters<I: ElfIds> {
    fn section(&self) -> &I::SectionId;
    fn offset(&self) -> u32;
}

pub trait ReprIdGetters {
    fn repr_id(&self) -> String;
}

#[derive(Debug)]
pub struct Ids;

impl ElfIds for Ids {
    type SectionId = ElfSectionId;
    type SymbolId = ElfSymbolId;
    type StringId = ElfStringId;
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ElfSectionId {
    pub index: u32,
}

impl std::fmt::Debug for ElfSectionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "section#{}", self.index)
    }
}

impl ReprIdGetters for ElfSectionId {
    fn repr_id(&self) -> String {
        format!("{}", self.index)
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

impl ReprIdGetters for ElfSymbolId {
    fn repr_id(&self) -> String {
        format!("{}#{}", self.section.repr_id(), self.index)
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

impl StringIdGetters<Ids> for ElfStringId {
    fn section(&self) -> &ElfSectionId {
        &self.section
    }

    fn offset(&self) -> u32 {
        self.offset
    }
}
