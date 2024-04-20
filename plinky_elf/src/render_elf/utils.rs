use crate::ids::serial::{SectionId, SerialIds, StringId, SymbolId};
use crate::ids::{ReprIdGetters, StringIdGetters};
use crate::{ElfObject, ElfPermissions, ElfSectionContent};
use plinky_diagnostics::widgets::Widget;
use plinky_diagnostics::WidgetWriter;

pub(super) fn render_perms(perms: &ElfPermissions) -> String {
    let mut output = String::new();
    let mut push = |cond: bool, chr: char| output.push(cond.then(|| chr).unwrap_or(' '));

    push(perms.read, 'R');
    push(perms.write, 'W');
    push(perms.execute, 'X');

    if output.trim().is_empty() {
        format!("{:1$}", "-", output.len())
    } else {
        output
    }
}

pub(super) fn section_name(object: &ElfObject<SerialIds>, id: SectionId) -> String {
    let section = object.sections.get(&id).expect("invalid section id");
    format!("{}#{}", resolve_string(object, section.name), id.repr_id())
}

pub(super) fn symbol_name(
    object: &ElfObject<SerialIds>,
    symbol_table_id: SectionId,
    id: SymbolId,
) -> String {
    let symbol_table = object.sections.get(&symbol_table_id).expect("invalid symbol table id");
    let ElfSectionContent::SymbolTable(symbol_table) = &symbol_table.content else {
        panic!("symbol table id is not a symbol table");
    };
    let symbol = symbol_table.symbols.get(&id).expect("invalid symbol id");
    format!("{}#{}", resolve_string(object, symbol.name), id.repr_id())
}

pub(super) fn resolve_string(object: &ElfObject<SerialIds>, id: StringId) -> &str {
    let table = object.sections.get(&id.section()).expect("invalid string section id");
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
