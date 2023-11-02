pub mod serial;
mod convert;
mod string;

pub use convert::{ConvertibleElfIds, IdConversionMap};
pub use string::StringIds;
pub(crate) use convert::convert;

use std::hash::Hash;
use std::fmt::Debug;

pub trait ElfIds: Debug {
    type SectionId: Debug + Clone + Hash + PartialEq + Eq + PartialOrd + Ord;
    type SymbolId: Debug + Clone + Hash + PartialEq + Eq + PartialOrd + Ord;
    type StringId: Debug + Clone + Hash + PartialEq + Eq + PartialOrd + Ord;
}

pub trait StringIdGetters<I: ElfIds> {
    fn section(&self) -> &I::SectionId;
    fn offset(&self) -> u32;
}
