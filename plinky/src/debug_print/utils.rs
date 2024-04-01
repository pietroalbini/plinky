use crate::repr::object::Object;
use crate::repr::symbols::SymbolValue;
use plinky_elf::ids::serial::{SectionId, SymbolId};
use plinky_elf::ElfPermissions;

pub(super) fn permissions(perms: &ElfPermissions) -> String {
    let mut output = String::new();
    if perms.read {
        output.push('r');
    }
    if perms.write {
        output.push('w');
    }
    if perms.execute {
        output.push('x');
    }
    if output.is_empty() {
        "no perms".into()
    } else {
        format!("perms: {output}")
    }
}

pub(super) fn section_name(object: &Object, id: SectionId) -> String {
    object
        .sections
        .get(id)
        .map(|section| section.name)
        .or_else(|| object.sections.name_of_removed_section(id))
        .map(|name| format!("{}#{}", name.resolve(), id.idx()))
        .unwrap_or_else(|| "<unknown section>".into())
}

pub(super) fn symbol_name(object: &Object, id: SymbolId) -> String {
    let symbol = object.symbols.get(id);
    let name = symbol.name.resolve();
    match (name.as_str(), &symbol.value) {
        ("", SymbolValue::SectionRelative { section, offset: 0 }) => {
            format!("<section {}>", section_name(object, *section))
        }
        ("", _) => format!("<symbol#{}>", symbol.id.idx()),
        (name, _) => format!("{}#{}", name, symbol.id.idx()),
    }
}
