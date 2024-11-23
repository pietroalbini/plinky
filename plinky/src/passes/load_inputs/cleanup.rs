use crate::interner::intern;
use crate::repr::object::Object;
use crate::repr::sections::SectionContent;
use crate::repr::symbols::views::AllSymbols;
use crate::repr::symbols::SymbolValue;
use plinky_utils::ints::ExtractNumber;
use std::collections::HashSet;

pub(super) fn run(object: &mut Object) {
    let gnu_stack = intern(".note.GNU-stack");

    let mut removed_gnu_stack = false;
    let mut sections_to_remove = Vec::new();
    for section in object.sections.iter() {
        if section.name == gnu_stack {
            sections_to_remove.push(section.id);
            removed_gnu_stack = true;
            continue;
        }
        if let SectionContent::Data(data) = &section.content {
            if data.bytes.is_empty() {
                sections_to_remove.push(section.id);
                continue;
            }
        }
        if let SectionContent::Uninitialized(uninit) = &section.content {
            if uninit.len.extract() == 0 {
                sections_to_remove.push(section.id);
                continue;
            }
        }
    }

    // Buggy compilers (hi Nora) could generate symbols pointing to empty sections, which would
    // later cause problems as those symbols would point to removed sections if we were to clean
    // them up. To avoid that, let's keep track of which sections have symbols and not delete them.
    let sections_referenced_by_symbols = object
        .symbols
        .iter(&AllSymbols)
        .filter_map(|symbol| match symbol.value() {
            SymbolValue::Absolute { .. }
            | SymbolValue::ExternallyDefined
            | SymbolValue::SectionNotLoaded
            | SymbolValue::Undefined
            | SymbolValue::Null => None,
            SymbolValue::SectionRelative { section, .. }
            | SymbolValue::SectionVirtualAddress { section, .. } => Some(section),
        })
        .collect::<HashSet<_>>();

    object.gnu_stack_section_ignored |= removed_gnu_stack;
    for id in sections_to_remove {
        if !sections_referenced_by_symbols.contains(&id) {
            object.sections.remove(id, Some(&mut object.symbols));
        }
    }
}
