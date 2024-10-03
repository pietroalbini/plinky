use crate::repr::object::Object;
use crate::repr::sections::SectionContent;
use crate::repr::symbols::views::{AllSymbols, DynamicSymbolTable};
use crate::repr::symbols::{SymbolId, SymbolValue};
use plinky_diagnostics::ObjectSpan;
use plinky_elf::ids::serial::SectionId;
use std::collections::{BTreeMap, BTreeSet};

pub(crate) fn run(object: &mut Object) -> Vec<RemovedSection> {
    let mut visitor = Visitor {
        symbols_to_sections: object
            .symbols
            .iter(&AllSymbols)
            .filter_map(|symbol| match symbol.value() {
                SymbolValue::SectionRelative { section, .. } => Some((symbol.id(), section)),
                SymbolValue::SectionVirtualAddress { section, .. } => Some((symbol.id(), section)),
                SymbolValue::Absolute { .. } => None,
                SymbolValue::Undefined => None,
                SymbolValue::Null => None,
            })
            .collect(),
        to_save: BTreeSet::new(),
        queue: BTreeSet::new(),
    };

    // Mark all symbols to be exported in .dynsym as a GC root, to avoid literally everything being
    // discarded when building shared libraries.
    for symbol in object.symbols.iter(&DynamicSymbolTable) {
        visitor.add(symbol.id());
    }

    // Mark all sections that will not be allocated in memory to be saved, as checking the
    // relocations from the entry point is not accurate for that.
    for section in object.sections.iter() {
        match &section.content {
            SectionContent::Data(data) if data.perms.read => {}
            SectionContent::Uninitialized(uninit) if uninit.perms.read => {}
            _ => {
                visitor.to_save.insert(section.id);
            }
        }
    }

    if let Some(entry_point) = object.entry_point {
        visitor.add(entry_point);
    }
    visitor.process(object);

    let mut removed_sections = Vec::new();
    let all_sections = object.sections.iter().map(|s| s.id).collect::<Vec<_>>();
    for section_id in all_sections {
        if !visitor.to_save.contains(&section_id) {
            if let Some(removed) = object.sections.remove(section_id, Some(&mut object.symbols)) {
                removed_sections.push(RemovedSection { id: section_id, source: removed.source });
            }
        }
    }
    removed_sections
}

pub(crate) struct RemovedSection {
    pub(crate) id: SectionId,
    pub(crate) source: ObjectSpan,
}

struct Visitor {
    symbols_to_sections: BTreeMap<SymbolId, SectionId>,
    to_save: BTreeSet<SectionId>,
    queue: BTreeSet<SectionId>,
}

impl Visitor {
    fn add(&mut self, symbol: SymbolId) {
        if let Some(&section_id) = self.symbols_to_sections.get(&symbol) {
            if !self.to_save.contains(&section_id) {
                self.queue.insert(section_id);
            }
        }
    }

    fn process(&mut self, object: &Object) {
        while let Some(section_id) = self.queue.pop_first() {
            self.to_save.insert(section_id);
            if let Some(section) = object.sections.get(section_id) {
                match &section.content {
                    SectionContent::Data(data) => {
                        for relocation in &data.relocations {
                            self.add(relocation.symbol);
                        }
                    }
                    SectionContent::Uninitialized(_) => {}
                    SectionContent::Strings(_) => {}
                    SectionContent::Symbols(_) => {}
                    SectionContent::SysvHash(_) => {}
                    SectionContent::Relocations(_) => {}
                    SectionContent::Dynamic(_) => {}
                    SectionContent::SectionNames => {}
                }
            }
        }
    }
}
