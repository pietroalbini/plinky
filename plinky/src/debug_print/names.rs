use crate::repr::object::Object;
use crate::repr::symbols::views::AllSymbols;
use crate::repr::symbols::SymbolValue;
use plinky_elf::ids::serial::{SectionId, SymbolId};
use plinky_utils::ints::ExtractNumber;
use std::collections::{BTreeMap, HashMap};
use std::hash::Hash;

pub(super) struct Names {
    sections: HashMap<SectionId, String>,
    symbols: HashMap<SymbolId, String>,
}

impl Names {
    pub(super) fn new(object: &Object) -> Self {
        let sections = calculate_names(
            object
                .sections
                .iter()
                .map(|section| (section.id, section.name))
                .chain(object.sections.names_of_removed_sections())
                .map(|(id, name)| (id, id, name.resolve().to_string())),
        );

        let symbols =
            calculate_names(object.symbols.iter_with_redirects(&AllSymbols).map(|(id, symbol)| {
                let name = match (symbol.name().resolve().as_str(), symbol.value()) {
                    ("", SymbolValue::SectionRelative { section, offset })
                        if offset.extract() == 0 =>
                    {
                        format!("<section {}>", sections[&section])
                    }
                    ("", _) => "<empty>".to_string(),
                    (name, _) => name.to_string(),
                };
                // We group by the *actual* symbol ID (after resolving redirects).
                (symbol.id(), id, name)
            }));

        Names { sections, symbols }
    }

    pub(super) fn section(&self, id: SectionId) -> &str {
        &self.sections[&id]
    }

    pub(super) fn symbol(&self, id: SymbolId) -> &str {
        &self.symbols[&id]
    }
}

fn calculate_names<K, I>(iter: I) -> HashMap<K, String>
where
    I: Iterator<Item = (K, K, String)>,
    K: Hash + Eq + Ord + Copy,
{
    let mut grouped = BTreeMap::new();
    for (group_id, id, name) in iter {
        grouped
            .entry(name)
            .or_insert_with(BTreeMap::new)
            .entry(group_id)
            .or_insert_with(Vec::new)
            .push(id);
    }

    let mut result = HashMap::new();
    for (name, group) in grouped {
        if group.len() == 1 {
            for id in group.values().next().unwrap() {
                result.insert(*id, name.clone());
            }
        } else {
            for (index, ids) in group.into_values().enumerate() {
                for id in ids {
                    result.insert(id, format!("{name}#{index}"));
                }
            }
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_names() {
        let names = calculate_names(
            [
                (0, 0, "Pietro"),
                (1, 1, "world"),
                (2, 2, "hello"),
                (3, 3, "world"),
                (4, 4, "hello"),
                (4, 5, "hello"),
            ]
            .into_iter()
            .map(|(group, id, name)| (group, id, name.to_string())),
        );

        assert_eq!(names[&0], "Pietro");
        assert_eq!(names[&1], "world#0");
        assert_eq!(names[&2], "hello#0");
        assert_eq!(names[&3], "world#1");
        assert_eq!(names[&4], "hello#1");
        assert_eq!(names[&5], "hello#1");
    }
}
