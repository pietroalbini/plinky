use crate::interner::intern;
use crate::passes::load_inputs::section_groups::SectionGroups;
use crate::repr::object::Object;
use crate::repr::sections::SectionContent;
use crate::repr::symbols::views::AllSymbols;
use crate::repr::symbols::SymbolValue;
use plinky_utils::ints::ExtractNumber;

pub(super) fn run(object: &mut Object, section_groups: &SectionGroups) {
    let gnu_stack = intern(".note.GNU-stack");

    let mut removed_gnu_stack = false;
    let mut sections_to_remove = Vec::new();
    for section in object.sections.iter() {
        if section.name == gnu_stack {
            sections_to_remove.push(section.id);
            removed_gnu_stack = true;
        }
        if let SectionContent::Data(data) = &section.content {
            if data.bytes.is_empty() {
                sections_to_remove.push(section.id);
            }
        }
        if let SectionContent::Uninitialized(uninit) = &section.content {
            if uninit.len.extract() == 0 {
                sections_to_remove.push(section.id);
            }
        }
    }

    let mut symbols_to_remove = Vec::new();
    for symbol in object.symbols.iter(&AllSymbols) {
        // GNU AS generates symbols for each section group, pointing to the SHT_GROUP. This is not
        // really useful, as nothing can refer to that section and the SHT_GROUP wouldn't be loaded
        // in memory anyway. To avoid the linker crashing when it sees a symbol to the section that
        // wasn't loaded, we remove all symbols pointing to a SHT_GROUP.
        let SymbolValue::SectionRelative { section, .. } = symbol.value() else { continue };
        if section_groups.is_section_a_group_definition(section) {
            symbols_to_remove.push(symbol.id());
        }
    }
    for symbol_id in symbols_to_remove {
        object.symbols.remove(symbol_id);
    }

    object.gnu_stack_section_ignored |= removed_gnu_stack;
    for id in sections_to_remove {
        object.sections.remove(id, Some(&mut object.symbols));
    }
}
