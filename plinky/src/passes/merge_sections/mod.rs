mod deduplicate;
mod rewrite;

use crate::passes::merge_sections::deduplicate::DeduplicationError;
use crate::repr::object::Object;
use plinky_macros::{Display, Error};
use crate::passes::merge_sections::rewrite::RewriteError;

pub(crate) fn run(object: &mut Object) -> Result<(), MergeSectionsError> {
    let deduplications = deduplicate::run(object)?;
    rewrite::run(object, deduplications)?;
    Ok(())
}

#[derive(Debug, Error, Display)]
pub(crate) enum MergeSectionsError {
    #[transparent]
    Deduplication(DeduplicationError),
    #[transparent]
    Rewrite(RewriteError),
}
