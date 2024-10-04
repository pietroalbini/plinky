use crate::ids::{ElfSectionId, ElfSymbolId};
use crate::render_elf::utils::resolve_string;
use crate::{ElfObject, ElfSectionContent, ElfSymbolDefinition};
use std::collections::{BTreeMap, HashMap};
use std::hash::Hash;

pub(super) struct Names {
    sections: HashMap<ElfSectionId, String>,
    symbols: HashMap<ElfSymbolId, String>,
}

impl Names {
    pub(super) fn new(object: &ElfObject) -> Self {
        let sections = calculate_names(object.sections.iter().map(|(id, section)| {
            let mut name = resolve_string(object, section.name).to_string();
            if name.is_empty() {
                name = "<empty>".to_string();
            }
            (*id, name)
        }));

        let symbols = calculate_names(
            object
                .sections
                .values()
                .filter_map(|section| match &section.content {
                    ElfSectionContent::SymbolTable(symbols) => Some(symbols),
                    _ => None,
                })
                .flat_map(|symbols| symbols.symbols.iter())
                .map(|(id, symbol)| {
                    let mut string = resolve_string(object, symbol.name).to_string();
                    if string.is_empty() {
                        string = match (&symbol.definition, symbol.value) {
                            (ElfSymbolDefinition::Section(section), 0) => {
                                format!("<section {}>", sections.get(&section).unwrap())
                            }
                            _ => "<empty>".to_string(),
                        };
                    }
                    (*id, string)
                }),
        );

        Names { sections, symbols }
    }

    pub(super) fn section(&self, id: ElfSectionId) -> &str {
        &self.sections[&id]
    }

    pub(super) fn symbol(&self, id: ElfSymbolId) -> &str {
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
