use crate::ids::ElfIds;
use crate::render_elf::utils::{render_perms, section_name};
use crate::{ElfObject, ElfSegmentContent, ElfSegmentType};
use plinky_diagnostics::widgets::{Table, Text, Widget};

pub(super) fn render_segments<I: ElfIds>(object: &ElfObject<I>) -> Box<dyn Widget> {
    if object.segments.is_empty() {
        return Box::new(Text::new("No segments in the ELF file."));
    }

    let mut table = Table::new();
    table.set_title("Segments:");
    table.add_row(["Type", "Perms", "Aligment", "Content"]);
    for segment in &object.segments {
        table.add_row([
            match segment.type_ {
                ElfSegmentType::Null => "Null".into(),
                ElfSegmentType::Load => "Load".into(),
                ElfSegmentType::Dynamic => "Dynamic".into(),
                ElfSegmentType::Interpreter => "Interpreter".into(),
                ElfSegmentType::Note => "Note".into(),
                ElfSegmentType::ProgramHeaderTable => "Program header table".into(),
                ElfSegmentType::GnuStack => "GNU stack".into(),
                ElfSegmentType::Unknown(id) => format!("<unknown: {id:#x}>"),
            },
            render_perms(&segment.perms),
            format!("{:#x}", segment.align),
            match &segment.content {
                ElfSegmentContent::Empty => "-".into(),
                ElfSegmentContent::Sections(sections) => sections
                    .iter()
                    .map(|id| section_name(object, id))
                    .collect::<Vec<_>>()
                    .join(", "),
                ElfSegmentContent::Unknown(unknown) => format!(
                    "file: {:#x} (len: {:#x}), memory: {:#x} (len: {:#x})",
                    unknown.file_offset,
                    unknown.file_size,
                    unknown.virtual_address,
                    unknown.memory_size
                ),
            },
        ]);
    }
    Box::new(table)
}
