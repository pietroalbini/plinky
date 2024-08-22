pub(crate) mod asm;
pub(crate) mod ld;
pub(crate) mod c;

use crate::template::Template;
use std::fmt::Debug;
use crate::TestContext;
use anyhow::Error;

pub trait Step: Debug + Send + Sync {
    fn run(&self, ctx: TestContext<'_>) -> Result<(), Error>;
    fn templates(&self) -> Vec<Template>;
}
