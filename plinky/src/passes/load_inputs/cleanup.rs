use crate::interner::intern;
use crate::repr::object::Object;
use crate::repr::sections::SectionContent;
use plinky_utils::ints::ExtractNumber;

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

    object.gnu_stack_section_ignored |= removed_gnu_stack;
    for id in sections_to_remove {
        object.sections.remove(id, Some(&mut object.symbols));
    }
}
