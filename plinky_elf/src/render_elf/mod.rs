pub use crate::render_elf::filters::{RenderElfFilters, RenderElfFiltersParseError};

use crate::ElfObject;
use crate::render_elf::names::Names;
use crate::render_elf::utils::{MultipleWidgets, resolve_string};
use plinky_diagnostics::widgets::Widget;

mod filters;
mod meta;
mod names;
mod sections;
mod segments;
mod utils;

pub use sections::render_note;

pub fn render(object: &ElfObject, filters: &RenderElfFilters) -> impl Widget + use<> {
    let names = Names::new(object);

    let mut widgets: Vec<Box<dyn Widget>> = Vec::new();
    if filters.meta {
        widgets.push(Box::new(meta::render_meta(object)));
    }
    for (id, section) in &object.sections {
        if filters.section(resolve_string(object, section.name)) {
            widgets.push(Box::new(sections::render_section(&names, object, *id, section)));
        }
    }
    if filters.segments {
        widgets.push(Box::new(segments::render_segments(object)));
    }
    MultipleWidgets(widgets)
}
