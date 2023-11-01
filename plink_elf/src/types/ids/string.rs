use super::IdConversionMap;
use crate::ids::{ConvertibleElfIds, ElfIds};
use crate::{Object, SectionContent};

#[derive(Debug)]
pub struct StringIds(());

impl StringIds {
    pub fn new() -> Self {
        Self(())
    }
}

impl ElfIds for StringIds {
    type SectionId = String;
    type SymbolId = String;
}

impl ConvertibleElfIds for StringIds {
    fn create_conversion_map<F>(&mut self, object: &Object<F>) -> IdConversionMap<F, Self>
    where
        F: ElfIds,
        Self: Sized,
    {
        let mut map = IdConversionMap::new();

        for (i, (id, section)) in object.sections.iter().enumerate() {
            map.section_ids.insert(
                id.clone(),
                format!(
                    "{} {}",
                    format_number(i, object.sections.len()),
                    section.name
                ),
            );

            match &section.content {
                SectionContent::SymbolTable(table) => {
                    for (i, (id, symbol)) in table.symbols.iter().enumerate() {
                        map.symbol_ids.insert(
                            id.clone(),
                            format!(
                                "{} {}",
                                format_number(i, table.symbols.len()),
                                if symbol.name.is_empty() {
                                    "<empty>"
                                } else {
                                    &symbol.name
                                }
                            ),
                        );
                    }
                }
                _ => {}
            }
        }

        map
    }
}

fn format_number(number: usize, total: usize) -> String {
    let total_len = total.to_string().len();
    format!("#{number:0>total_len$}")
}
