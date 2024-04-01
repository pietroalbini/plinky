use crate::debug_print::utils::section_name;
use crate::passes::deduplicate::Deduplication;
use crate::passes::layout::{Layout, SectionLayout, SegmentType};
use crate::repr::object::Object;
use plinky_diagnostics::widgets::{Table, Widget};
use plinky_diagnostics::{Diagnostic, DiagnosticKind};
use plinky_elf::ids::serial::SectionId;

pub(super) fn render_layout(object: &Object, layout: &Layout) -> Diagnostic {
    let mut sections = Table::new();
    sections.set_title("Sections:");
    sections.add_row(["Section", "Source object", "Memory address"]);

    let mut sections_content = Vec::new();
    for section in object.sections.iter() {
        sections_content.push((section.id, &section.source));
    }
    sections_content.sort_by_key(|(id, _)| (layout.of_section(*id), *id));
    for (id, source) in sections_content {
        sections.add_row([
            section_name(object, id),
            source.to_string(),
            match layout.of_section(id) {
                SectionLayout::Allocated { address } => format!("{address:#x}"),
                SectionLayout::NotAllocated => "not allocated".to_string(),
            },
        ]);
    }

    let mut segments = Table::new();
    segments.set_title("Segments:");
    segments.add_row(["Start", "Size", "Align", "Type", "Permissions", "Sections"]);
    for segment in layout.iter_segments() {
        segments.add_row([
            format!("{:#x}", segment.start),
            format!("{:#x}", segment.len),
            format!("{:#x}", segment.align),
            match segment.type_ {
                SegmentType::Program => "program".into(),
                SegmentType::Uninitialized => "uninit".into(),
            },
            format!("{:?}", segment.perms),
            segment
                .sections
                .iter()
                .map(|id| section_name(object, *id))
                .collect::<Vec<_>>()
                .join("\n"),
        ]);
    }

    Diagnostic::new(DiagnosticKind::DebugPrint, "calculated layout")
        .add(sections)
        .add(segments)
        .add_iter(
            layout
                .iter_deduplications()
                .map(|(id, deduplication)| render_deduplication(object, id, deduplication)),
        )
}

fn render_deduplication(
    object: &Object,
    id: SectionId,
    deduplication: &Deduplication,
) -> Box<dyn Widget> {
    let target = section_name(object, deduplication.target);

    let mut table = Table::new();
    table.set_title(format!(
        "deduplication facade {} in {}",
        section_name(object, id),
        deduplication.source
    ));
    table.add_row(["From", "To"]);
    for (from, to) in &deduplication.map {
        table.add_row([format!("{from:#x}"), format!("{target} + {to:#x}")]);
    }

    Box::new(table)
}
