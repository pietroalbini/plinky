use crate::template::{TemplateContext, Value};
use std::path::PathBuf;

pub(crate) fn register_builtins(ctx: &mut TemplateContext) {
    ctx.add_function("dirname", dirname);
}

fn dirname(path: PathBuf) -> Value {
    Value::Path(path.parent().expect("directory has no parent").into())
}
