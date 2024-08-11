use crate::debug_print::utils::section_name;
use crate::passes::deduplicate::Deduplication;
use crate::passes::layout::Layout;
use crate::repr::object::Object;
use crate::repr::segments::{SegmentContent, SegmentStart, SegmentType};
use plinky_diagnostics::widgets::{Table, Widget};
use plinky_diagnostics::{Diagnostic, DiagnosticKind};
use plinky_elf::ids::serial::SectionId;
use plinky_elf::writer::layout::Part;
use std::collections::BTreeMap;

pub(super) fn render_layout(
    object: &Object,
    layout: &Layout,
    deduplications: &BTreeMap<SectionId, Deduplication>,
) -> Diagnostic {
    let mut table = Table::new();
    table.set_title("Layout:");
    table.add_row(["Part", "File offset", "File length", "Memory address", "Memory length"]);

    for part in layout.parts() {
        let meta = layout.metadata(part);
        table.add_row([
            match part {
                Part::Header => "<elf header>".into(),
                Part::SectionHeaders => "<section header>".into(),
                Part::ProgramHeaders => "<program header>".into(),
                Part::Padding { .. } => "<padding>".into(),
                Part::ProgramSection(id)
                | Part::UninitializedSection(id)
                | Part::StringTable(id)
                | Part::SymbolTable(id)
                | Part::Hash(id)
                | Part::Rel(id)
                | Part::Rela(id)
                | Part::Group(id)
                | Part::Dynamic(id) => section_name(object, *id),
            },
            meta.file.as_ref().map(|m| m.offset.to_string()).unwrap_or_else(|| "-".into()),
            meta.file.as_ref().map(|m| m.len.to_string()).unwrap_or_else(|| "-".into()),
            meta.memory.as_ref().map(|m| m.address.to_string()).unwrap_or_else(|| "-".into()),
            meta.memory.as_ref().map(|m| m.len.to_string()).unwrap_or_else(|| "-".into()),
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
        .add(table)
        .add(segments)
        .add_iter(
            deduplications
                .iter()
                .map(|(id, deduplication)| render_deduplication(object, *id, deduplication)),
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
