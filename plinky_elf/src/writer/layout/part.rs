use plinky_macros::{Display, Error};
use plinky_utils::ints::{Address, Length, Offset, OutOfBoundsError};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
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
    Note(S),
    GnuHash(S),
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
            Part::GnuHash(id) => Some(id),
            Part::Rel(id) => Some(id),
            Part::Rela(id) => Some(id),
            Part::Group(id) => Some(id),
            Part::Dynamic(id) => Some(id),
            Part::Note(id) => Some(id),
            Part::Padding { .. } => None,
        }
    }

    pub fn convert_ids<T>(self, map: &BTreeMap<S, T>) -> Part<T>
    where
        S: Ord + Eq,
        T: Clone,
    {
        let c = |id| map.get(&id).unwrap().clone();
        match self {
            Part::Header => Part::Header,
            Part::SectionHeaders => Part::SectionHeaders,
            Part::ProgramHeaders => Part::ProgramHeaders,
            Part::ProgramSection(id) => Part::ProgramSection(c(id)),
            Part::UninitializedSection(id) => Part::UninitializedSection(c(id)),
            Part::StringTable(id) => Part::StringTable(c(id)),
            Part::SymbolTable(id) => Part::SymbolTable(c(id)),
            Part::Hash(id) => Part::Hash(c(id)),
            Part::GnuHash(id) => Part::GnuHash(c(id)),
            Part::Rel(id) => Part::Rel(c(id)),
            Part::Rela(id) => Part::Rela(c(id)),
            Part::Group(id) => Part::Group(c(id)),
            Part::Dynamic(id) => Part::Dynamic(c(id)),
            Part::Note(id) => Part::Note(c(id)),
            Part::Padding { id, len } => Part::Padding { id, len },
        }
    }

    pub(super) fn present_in_file(&self) -> bool {
        match self {
            Part::UninitializedSection(_) => false,
            _ => true,
        }
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub struct PaddingId(u64);

impl PaddingId {
    pub fn next() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        PaddingId(COUNTER.fetch_add(1, Ordering::Relaxed))
    }
}

#[derive(Debug, Clone)]
pub struct PartMetadata {
    pub file: Option<PartFile>,
    pub memory: Option<PartMemory>,
}

impl PartMetadata {
    pub const EMPTY: Self = Self { file: None, memory: None };

    pub fn add(&self, other: &PartMetadata) -> Result<PartMetadata, MergePartMetadataError> {
        let file = match (&self.file, &other.file) {
            (None, None) => None,
            (Some(a), Some(b)) => {
                if a.offset.add(a.len.as_offset()?)? != b.offset {
                    return Err(MergePartMetadataError::NotAdjacent);
                }
                Some(PartFile { len: a.len.add(b.len)?, offset: a.offset })
            }
            (None, Some(_)) | (Some(_), None) => return Err(MergePartMetadataError::MixingInFile),
        };

        let memory = match (&self.memory, &other.memory) {
            (None, None) => None,
            (Some(a), Some(b)) => {
                if a.address.offset(a.len.as_offset()?)? != b.address {
                    return Err(MergePartMetadataError::NotAdjacent);
                }
                Some(PartMemory { len: a.len.add(b.len)?, address: a.address })
            }
            (None, Some(_)) | (Some(_), None) => {
                return Err(MergePartMetadataError::MixingInMemory);
            }
        };

        Ok(PartMetadata { file, memory })
    }
}

#[derive(Debug, Clone)]
pub struct PartFile {
    pub len: Length,
    pub offset: Offset,
}

#[derive(Debug, Clone)]
pub struct PartMemory {
    pub len: Length,
    pub address: Address,
}

#[derive(Debug, Error, Display)]
pub enum MergePartMetadataError {
    #[display("the two parts are not adjacent")]
    NotAdjacent,
    #[display("cannot mix parts present and missing in the file")]
    MixingInFile,
    #[display("cannot mix parts present and missing in the memory")]
    MixingInMemory,
    #[transparent]
    OutOfBounds(OutOfBoundsError),
}
