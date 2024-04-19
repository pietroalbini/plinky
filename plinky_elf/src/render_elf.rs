use crate::ids::serial::{SectionId, SerialIds};
use crate::ids::StringIdGetters;
use crate::{
    ElfABI, ElfClass, ElfEndian, ElfMachine, ElfObject, ElfPermissions, ElfSection,
    ElfSectionContent, ElfSegmentContent, ElfSegmentType, ElfType,
};
use plinky_diagnostics::widgets::{Table, Text, Widget, WidgetGroup};
use plinky_diagnostics::WidgetWriter;

pub fn render_elf(object: &ElfObject<SerialIds>) -> impl Widget {
    let mut widgets: Vec<Box<dyn Widget>> = Vec::new();
    widgets.push(Box::new(render_meta(object)));
    for (&id, section) in &object.sections {
        widgets.push(Box::new(render_section(object, id, section)));
    }
    widgets.push(Box::new(render_segments(object)));
    MultipleWidgets(widgets)
}

fn render_meta(object: &ElfObject<SerialIds>) -> impl Widget {
    let mut table = Table::new();
    table.set_title("Metadata:");
    table.add_row([
        "Class",
        match object.env.class {
            ElfClass::Elf32 => "ELF 32bit",
            ElfClass::Elf64 => "ELF 64bit",
        },
    ]);
    table.add_row([
        "Endian",
        match object.env.endian {
            ElfEndian::Little => "Little",
        },
    ]);
    table.add_row([
        "ABI",
        match object.env.abi {
            ElfABI::SystemV => "System V",
        },
    ]);
    table.add_row([
        "Machine",
        match object.env.machine {
            ElfMachine::X86 => "x86",
            ElfMachine::X86_64 => "x86-64",
        },
    ]);
    table.add_row([
        "Type",
        match object.type_ {
            ElfType::Relocatable => "Relocatable",
            ElfType::Executable => "Executable",
            ElfType::SharedObject => "Shared object",
            ElfType::Core => "Core dump",
        },
    ]);
    table.add_row([
        "Entrypoint".to_string(),
        match object.entry {
            Some(entry) => entry.to_string(),
            None => "-".to_string(),
        },
    ]);
    table
}

fn render_section(
    object: &ElfObject<SerialIds>,
    id: SectionId,
    section: &ElfSection<SerialIds>,
) -> impl Widget {
    let content: Box<dyn Widget> = match &section.content {
        //ElfSectionContent::Null => todo!(),
        //ElfSectionContent::Program(_) => todo!(),
        //ElfSectionContent::Uninitialized(_) => todo!(),
        //ElfSectionContent::SymbolTable(_) => todo!(),
        //ElfSectionContent::StringTable(_) => todo!(),
        //ElfSectionContent::RelocationsTable(_) => todo!(),
        //ElfSectionContent::Note(_) => todo!(),
        //ElfSectionContent::Unknown(_) => todo!(),
        _ => Box::new(Text::new(format!("{:#?}", section.content))),
    };

    WidgetGroup::new()
        .name(format!("section {} (address: {:#x})", section_name(object, id), section.memory_address))
        .add(content)
}

fn render_segments(object: &ElfObject<SerialIds>) -> impl Widget {
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
                    .map(|&id| section_name(object, id))
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
    table
}

fn render_perms(perms: &ElfPermissions) -> String {
    let mut output = String::new();
    let mut push = |cond: bool, chr: char| output.push(cond.then(|| chr).unwrap_or(' '));

    push(perms.read, 'R');
    push(perms.write, 'W');
    push(perms.execute, 'X');

    output
}

fn section_name(object: &ElfObject<SerialIds>, id: SectionId) -> String {
    let section = object.sections.get(&id).expect("invalid section id");
    let shstrtab = object.sections.get(&section.name.section()).expect("invalid string section id");
    let ElfSectionContent::StringTable(shstrtab) = &shstrtab.content else {
        panic!("string section id is not a string table");
    };
    let name = shstrtab.get(section.name.offset()).expect("missing section name");
    format!("{name}#{}", id.idx())
}

struct MultipleWidgets(Vec<Box<dyn Widget>>);

impl Widget for MultipleWidgets {
    fn render(&self, writer: &mut WidgetWriter<'_>) {
        for (i, widget) in self.0.iter().enumerate() {
            if i != 0 {
                writer.push_str("\n\n");
            }
            widget.render(writer);
        }
    }
}
