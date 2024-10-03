use crate::ids::{ElfIds, StringIdGetters};
use crate::{ElfObject, ElfPermissions, ElfSectionContent};
use plinky_diagnostics::widgets::Widget;
use plinky_diagnostics::WidgetWriter;

pub(super) fn render_perms(perms: &ElfPermissions) -> String {
    let mut output = String::new();
    let mut push = |cond: bool, chr: char| output.push(if cond { chr } else { ' ' });

    push(perms.read, 'R');
    push(perms.write, 'W');
    push(perms.execute, 'X');

    if output.trim().is_empty() {
        format!("{:1$}", "-", output.len())
    } else {
        output
    }
}

pub(super) fn resolve_string<'a, I: ElfIds>(object: &'a ElfObject<I>, id: &I::StringId) -> &'a str {
    let table = object.sections.get(id.section()).expect("invalid string section id");
    let ElfSectionContent::StringTable(table) = &table.content else {
        panic!("string section id is not a string table");
    };
    table.get(id.offset()).expect("missing string")
}

pub(super) struct MultipleWidgets(pub(super) Vec<Box<dyn Widget>>);

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
