pub use crate::render_elf::filters::{RenderElfFilters, RenderElfFiltersParseError};

use crate::ids::ElfIds;
use crate::render_elf::utils::{resolve_string, MultipleWidgets};
use crate::ElfObject;
use plinky_diagnostics::widgets::Widget;
use crate::render_elf::names::Names;

mod names;
mod filters;
mod meta;
mod sections;
mod segments;
mod utils;

pub fn render<I: ElfIds + 'static>(
    object: &ElfObject<I>,
    filters: &RenderElfFilters,
) -> impl Widget {
    let names = Names::new(object);

    let mut widgets: Vec<Box<dyn Widget>> = Vec::new();
    if filters.meta {
        widgets.push(Box::new(meta::render_meta(object)));
    }
    for (id, section) in &object.sections {
        if filters.section(resolve_string(object, &section.name)) {
            widgets.push(Box::new(sections::render_section(&names, object, id, section)));
        }
    }
    if filters.segments {
        widgets.push(Box::new(segments::render_segments(object)));
    }
    MultipleWidgets(widgets)
}
