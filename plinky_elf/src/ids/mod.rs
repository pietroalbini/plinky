pub mod convert;
pub mod serial;

pub use convert::convert;
pub use convert::{ConvertibleElfIds, IdConversionMap};

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
