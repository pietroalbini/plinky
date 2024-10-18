use crate::render_elf::utils::render_perms;
use crate::{ElfObject, ElfSegmentType};
use plinky_diagnostics::widgets::{Table, Text, Widget};

pub(super) fn render_segments(object: &ElfObject) -> Box<dyn Widget> {
    if object.segments.is_empty() {
        return Box::new(Text::new("No segments in the ELF file."));
    }

    let mut table = Table::new();
    table.set_title("Segments:");
    table.add_row([
        "Type",
        "Perms",
        "Aligment",
        "File offset",
        "File len",
        "Memory address",
        "Memory len",
    ]);
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
                ElfSegmentType::GnuRelro => "GNU relocations read-only".into(),
                ElfSegmentType::GnuProperty => "GNU property".into(),
                ElfSegmentType::Unknown(id) => format!("<unknown: {id:#x}>"),
            },
            render_perms(&segment.perms),
            format!("{:#x}", segment.align),
            format!("{:#x}", segment.file_offset),
            format!("{:#x}", segment.file_size),
            format!("{:#x}", segment.virtual_address),
            format!("{:#x}", segment.memory_size),
        ]);
    }
    Box::new(table)
}
