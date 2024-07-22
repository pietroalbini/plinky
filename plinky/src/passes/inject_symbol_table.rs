use crate::interner::intern;
use crate::repr::object::Object;
use crate::repr::sections::{Section, SectionContent, StringsForSymbolsSection, SymbolsSection};
use crate::repr::symbols::views::AllSymbols;
use plinky_diagnostics::ObjectSpan;
use plinky_elf::ids::serial::SerialIds;

pub(crate) fn run(object: &mut Object, ids: &mut SerialIds) {
    let string_table_id = ids.allocate_section_id();
    object.sections.add(Section {
        id: string_table_id,
        name: intern(".strtab"),
        source: ObjectSpan::new_synthetic(),
        content: SectionContent::StringsForSymbols(StringsForSymbolsSection {
            view: Box::new(AllSymbols),
        }),
    });
    object.sections.add(Section {
        id: ids.allocate_section_id(),
        name: intern(".symtab"),
        source: ObjectSpan::new_synthetic(),
        content: SectionContent::Symbols(SymbolsSection {
            strings: string_table_id,
            view: Box::new(AllSymbols),
        }),
    });
}
