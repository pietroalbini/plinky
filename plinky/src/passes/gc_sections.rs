use crate::repr::object::{Object, SectionContent};
use crate::repr::symbols::SymbolValue;
use plinky_elf::ids::serial::{SectionId, SymbolId};
use std::collections::{BTreeMap, BTreeSet};
use plinky_diagnostics::ObjectSpan;

pub(crate) fn run(object: &mut Object) -> Vec<RemovedSection> {
    let mut visitor = Visitor {
        symbols_to_sections: object
            .symbols
            .iters_with_redirects()
            .filter_map(|(id, symbol)| match symbol.value {
                SymbolValue::SectionRelative { section, .. } => Some((id, section)),
                SymbolValue::Absolute { .. } => None,
                SymbolValue::Undefined => None,
            })
            .collect(),
        visited: BTreeSet::new(),
        queue: BTreeSet::new(),
    };

    visitor.add(object.entry_point);
    visitor.process(object);

    let mut removed_sections = Vec::new();
    let all_sections = object.sections.iter().map(|s| s.id).collect::<Vec<_>>();
    for section_id in all_sections {
        if !visitor.visited.contains(&section_id) {
            if let Some(removed) = object.sections.remove(section_id) {
                removed_sections.push(RemovedSection {
                    id: section_id,
                    source: removed.source,
                });
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
    visited: BTreeSet<SectionId>,
    queue: BTreeSet<SectionId>,
}

impl Visitor {
    fn add(&mut self, symbol: SymbolId) {
        if let Some(&section_id) = self.symbols_to_sections.get(&symbol) {
            if !self.visited.contains(&section_id) {
                self.queue.insert(section_id);
            }
        }
    }

    fn process(&mut self, object: &Object) {
        while let Some(section_id) = self.queue.pop_first() {
            self.visited.insert(section_id);
            if let Some(section) = object.sections.get(section_id) {
                match &section.content {
                    SectionContent::Data(data) => {
                        for relocation in &data.relocations {
                            self.add(relocation.symbol);
                        }
                    },
                    SectionContent::Uninitialized(_) => {},
                }
            }
        }
    }
}
