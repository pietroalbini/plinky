use crate::debug_print::utils::section_name;
use crate::passes::deduplicate::Deduplication;
use crate::passes::layout::{Layout, SectionLayout};
use crate::repr::object::Object;
use crate::repr::segments::{SegmentContent, SegmentStart, SegmentType};
use plinky_diagnostics::widgets::{Table, Widget};
use plinky_diagnostics::{Diagnostic, DiagnosticKind};
use plinky_elf::ids::serial::SectionId;

pub(super) fn render_layout(object: &Object, layout: &Layout) -> Diagnostic {
    let mut sections = Table::new();
    sections.set_title("Sections:");
    sections.add_row(["Section", "Source object", "Memory address", "Length"]);

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
                SectionLayout::Allocated { address, .. } => format!("{address}"),
                SectionLayout::NotAllocated => "not allocated".to_string(),
            },
            match layout.of_section(id) {
                SectionLayout::Allocated { len, .. } => format!("{len:#x}"),
                SectionLayout::NotAllocated => "-".to_string(),
            },
        ]);
    }

    let mut segments = Table::new();
    segments.set_title("Segments:");
    segments.add_row(["Start", "Align", "Type", "Permissions", "Content"]);
    for segment in object.segments.iter() {
        segments.add_row([
            match segment.start(layout) {
                SegmentStart::Address(address) => format!("{address}"),
                SegmentStart::ProgramHeader => format!("<program header>"),
            },
            format!("{:#x}", segment.align),
            match segment.type_ {
                SegmentType::ProgramHeader => "program header".into(),
                SegmentType::Program => "program".into(),
                SegmentType::Uninitialized => "uninit".into(),
                SegmentType::Dynamic => "dynamic".into(),
                SegmentType::Interpreter => "interpreter".into(),
            },
            format!("{:?}", segment.perms),
            match &segment.content {
                SegmentContent::ElfHeader => "elf header".into(),
                SegmentContent::ProgramHeader => "program header".into(),
                SegmentContent::Sections(sections) => sections
                    .iter()
                    .map(|id| section_name(object, *id))
                    .collect::<Vec<_>>()
                    .join("\n"),
            },
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
        table.add_row([format!("{from}"), format!("{target} + {to}")]);
    }

    Box::new(table)
}
