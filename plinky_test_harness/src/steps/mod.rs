pub(crate) mod asm;
pub(crate) mod c;
pub(crate) mod ld;

use crate::template::Template;
use crate::TestContext;
use anyhow::Error;
use std::fmt::Debug;

pub trait Step: Debug + Send + Sync {
    fn run(&self, ctx: TestContext<'_>) -> Result<(), Error>;
    fn templates(&self) -> Vec<Template>;
}
