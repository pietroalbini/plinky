use crate::ids::serial::SerialIds;
use crate::render_elf::utils::{render_perms, section_name, MultipleWidgets};
use crate::ElfObject;
use plinky_diagnostics::widgets::Widget;

mod meta;
mod sections;
mod segments;
mod utils;

pub fn render(object: &ElfObject<SerialIds>) -> impl Widget {
    let mut widgets: Vec<Box<dyn Widget>> = Vec::new();
    widgets.push(Box::new(meta::render_meta(object)));
    for (&id, section) in &object.sections {
        widgets.push(Box::new(sections::render_section(object, id, section)));
    }
    widgets.push(Box::new(segments::render_segments(object)));
    MultipleWidgets(widgets)
}
