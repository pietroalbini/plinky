use crate::repr::object::Object;
use crate::repr::sections::SectionId;
use crate::repr::symbols::views::AllSymbols;
use crate::repr::symbols::{SymbolId, SymbolValue};
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
                .map(|(id, name)| (id, name.resolve().to_string())),
        );

        let symbols = calculate_names(object.symbols.iter(&AllSymbols).map(|symbol| {
            let name = match (symbol.name().resolve().as_str(), symbol.value()) {
                (_, SymbolValue::Section { section }) => {
                    format!("<section {}>", sections[&section])
                }
                ("", _) => "<empty>".to_string(),
                (name, _) => name.to_string(),
            };
            (symbol.id(), name)
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
    I: Iterator<Item = (K, String)>,
    K: Hash + Eq + Ord + Copy,
{
    let mut grouped = BTreeMap::new();
    for (id, name) in iter {
        grouped.entry(name).or_insert_with(Vec::new).push(id);
    }

    let mut result = HashMap::new();
    for (name, ids) in grouped {
        if ids.len() == 1 {
            result.insert(ids[0], name.clone());
        } else {
            for (index, id) in ids.iter().enumerate() {
                result.insert(*id, format!("{name}#{index}"));
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
            [("Pietro"), ("world"), ("hello"), ("world"), ("hello")]
                .into_iter()
                .map(String::from)
                .enumerate(),
        );

        assert_eq!(names[&0], "Pietro");
        assert_eq!(names[&1], "world#0");
        assert_eq!(names[&2], "hello#0");
        assert_eq!(names[&3], "world#1");
        assert_eq!(names[&4], "hello#1");
    }
}
