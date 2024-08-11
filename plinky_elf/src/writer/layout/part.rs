use plinky_utils::ints::{Address, ExtractNumber, Length, Offset};
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub enum Part<S> {
    Header,
    SectionHeaders,
    ProgramHeaders,
    ProgramSection(S),
    UninitializedSection(S),
    StringTable(S),
    SymbolTable(S),
    Hash(S),
    Rel(S),
    Rela(S),
    Group(S),
    Dynamic(S),
    Padding { id: PaddingId, len: usize },
}

impl<S> Part<S> {
    pub fn section_id(&self) -> Option<&S> {
        match self {
            Part::Header => None,
            Part::SectionHeaders => None,
            Part::ProgramHeaders => None,
            Part::ProgramSection(id) => Some(id),
            Part::UninitializedSection(id) => Some(id),
            Part::StringTable(id) => Some(id),
            Part::SymbolTable(id) => Some(id),
            Part::Hash(id) => Some(id),
            Part::Rel(id) => Some(id),
            Part::Rela(id) => Some(id),
            Part::Group(id) => Some(id),
            Part::Dynamic(id) => Some(id),
            Part::Padding { .. } => None,
        }
    }

    pub(super) fn present_in_file(&self) -> bool {
        match self {
            Part::UninitializedSection(_) => false,
            _ => true,
        }
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub struct PaddingId(u64);

impl PaddingId {
    pub fn next() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        PaddingId(COUNTER.fetch_add(1, Ordering::Relaxed))
    }
}

#[derive(Debug)]
pub struct PartMetadata {
    pub file: Option<PartFile>,
    pub memory: Option<PartMemory>,
}

impl PartMetadata {
    pub(crate) fn segment_bounds(&self) -> (u64, u64, u64, u64) {
        let (file_offset, file_len) = match &self.file {
            Some(file) => (file.offset.extract() as _, file.len.extract()),
            None => (0, 0),
        };

        let (memory_address, memory_len) = match &self.memory {
            Some(memory) => (memory.address.extract(), memory.len.extract()),
            None => (0, 0),
        };

        (file_offset, file_len, memory_address, memory_len)
    }
}

#[derive(Debug)]
pub struct PartFile {
    pub len: Length,
    pub offset: Offset,
}

#[derive(Debug)]
pub struct PartMemory {
    pub len: Length,
    pub address: Address,
}
