use crate::debug_print::names::Names;
use crate::repr::object::Object;
use crate::repr::sections::SectionId;
use crate::repr::segments::{SegmentContent, SegmentType};
use plinky_diagnostics::widgets::Table;
use plinky_diagnostics::{Diagnostic, DiagnosticKind};
use plinky_elf::writer::layout::{Layout, Part};
use plinky_utils::ints::ExtractNumber;

pub(super) fn render_layout(object: &Object, layout: &Layout<SectionId>) -> Diagnostic {
    let names = Names::new(object);

    let mut table = Table::new();
    table.set_title("Layout:");
    table.add_head(["Part", "File offset", "File length", "Memory address", "Memory length"]);

    for part in layout.parts() {
        let meta = layout.metadata(part);
        table.add_body([
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
                | Part::GnuHash(id)
                | Part::Rel(id)
                | Part::Rela(id)
                | Part::Group(id)
                | Part::Dynamic(id)
                | Part::Note(id) => names.section(*id).into(),
            },
            meta.file.as_ref().map(|m| m.offset.to_string()).unwrap_or_else(|| "-".into()),
            meta.file.as_ref().map(|m| m.len.to_string()).unwrap_or_else(|| "-".into()),
            meta.memory.as_ref().map(|m| m.address.to_string()).unwrap_or_else(|| "-".into()),
            meta.memory.as_ref().map(|m| m.len.to_string()).unwrap_or_else(|| "-".into()),
        ]);
    }

    let mut sorted_segments =
        object.segments.iter().map(|(_id, segment)| segment).collect::<Vec<_>>();
    sorted_segments.sort_by_key(|segment| {
        (
            &segment.type_,
            segment.layout(layout).memory.as_ref().map(|m| m.address.extract()).unwrap_or(0),
        )
    });

    let mut segments = Table::new();
    segments.set_title("Segments:");
    segments.add_head(["Start", "Align", "Type", "Perms", "Content"]);
    for segment in sorted_segments {
        segments.add_body([
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
                SegmentType::Notes => "notes".into(),
                SegmentType::GnuStack => "GNU stack".into(),
                SegmentType::GnuRelro => "GNU relro".into(),
                SegmentType::GnuProperty => "GNU property".into(),
            },
            segment.perms.to_string(),
            segment
                .content
                .iter()
                .map(|c| match c {
                    SegmentContent::ProgramHeader => "<program header>",
                    SegmentContent::ElfHeader => "<elf header>",
                    SegmentContent::Section(id) => names.section(*id),
                })
                .collect::<Vec<_>>()
                .join("\n"),
        ]);
    }

    Diagnostic::new(DiagnosticKind::DebugPrint, "calculated layout").add(table).add(segments)
}
