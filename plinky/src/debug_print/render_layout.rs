use crate::debug_print::utils::section_name;
use crate::passes::deduplicate::Deduplication;
use crate::repr::object::Object;
use crate::repr::segments::{SegmentContent, SegmentType};
use plinky_diagnostics::widgets::{Table, Widget};
use plinky_diagnostics::{Diagnostic, DiagnosticKind};
use plinky_elf::ids::serial::{SectionId, SerialIds};
use plinky_elf::writer::layout::{Layout, Part};
use std::collections::BTreeMap;

pub(super) fn render_layout(
    object: &Object,
    layout: &Layout<SerialIds>,
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
    segments.add_row(["Start", "Align", "Type", "Perms", "Content"]);
    for segment in object.segments.iter() {
        segments.add_row([
            segment
                .layout(layout)
                .memory
                .as_ref()
                .map(|m| m.address.to_string())
                .unwrap_or("-".into()),
            format!("{:#x}", segment.align),
            match segment.type_ {
                SegmentType::ProgramHeader => "program header".into(),
                SegmentType::Program => "program".into(),
                SegmentType::Uninitialized => "uninit".into(),
                SegmentType::Dynamic => "dynamic".into(),
                SegmentType::Interpreter => "interpreter".into(),
                SegmentType::GnuStack => "GNU stack".into(),
                SegmentType::GnuRelro => "GNU relro".into(),
            },
            segment.perms.to_string(),
            segment
                .content
                .iter()
                .map(|c| match c {
                    SegmentContent::ProgramHeader => "<program header>".into(),
                    SegmentContent::ElfHeader => "<elf header>".into(),
                    SegmentContent::Section(id) => section_name(object, *id),
                })
                .collect::<Vec<_>>()
                .join("\n"),
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
