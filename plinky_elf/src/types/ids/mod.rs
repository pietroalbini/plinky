mod convert;
pub mod serial;

pub use convert::convert;
pub use convert::{ConvertibleElfIds, IdConversionMap};

use std::fmt::Debug;
use std::hash::Hash;

pub trait ElfIds: Debug {
    type SectionId: Debug + Clone + Hash + PartialEq + Eq + PartialOrd + Ord;
    type SymbolId: Debug + Clone + Hash + PartialEq + Eq + PartialOrd + Ord;
    type StringId: Debug + Clone + Hash + PartialEq + Eq + PartialOrd + Ord;
}

pub trait StringIdGetters<I: ElfIds> {
    fn section(&self) -> &I::SectionId;
    fn offset(&self) -> u32;
}
