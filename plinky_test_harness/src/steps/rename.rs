use crate::template::{Template, Value};
use crate::{Step, TestContext};
use anyhow::Error;

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RenameStep {
    from: Template,
    to: Template,
}

impl Step for RenameStep {
    fn run(&self, ctx: TestContext<'_>) -> Result<(), Error> {
        let from = ctx.maybe_relative_to_src(self.from.resolve(&*ctx.template)?);
        let to = self.to.resolve(&*ctx.template)?;

        let dest = ctx.dest.join(ctx.step_name).join(to);
        std::fs::create_dir_all(dest.parent().unwrap())?;
        std::fs::copy(from, &dest)?;

        ctx.template.set_variable(ctx.step_name, Value::Path(dest));

        Ok(())
    }

    fn templates(&self) -> Vec<Template> {
        vec![self.from.clone(), self.to.clone()]
    }
}
