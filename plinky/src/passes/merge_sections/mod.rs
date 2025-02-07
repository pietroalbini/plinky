mod deduplicate;
mod rewrite;
mod same_name;

use crate::passes::merge_sections::deduplicate::DeduplicationError;
use crate::passes::merge_sections::rewrite::RewriteError;
use crate::repr::object::Object;
use plinky_macros::{Display, Error};
use crate::passes::merge_sections::same_name::MergeSameNameError;

pub(crate) fn run(object: &mut Object) -> Result<(), MergeSectionsError> {
    let deduplications = deduplicate::run(object)?;
    let same_name = same_name::run(object)?;
    rewrite::run(object, deduplications, same_name)?;
    Ok(())
}

#[derive(Debug, Error, Display)]
pub(crate) enum MergeSectionsError {
    #[transparent]
    Deduplication(DeduplicationError),
    #[transparent]
    MergeSameName(MergeSameNameError),
    #[transparent]
    Rewrite(RewriteError),
}
