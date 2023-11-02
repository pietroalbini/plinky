use super::IdConversionMap;
use crate::ids::{ConvertibleElfIds, ElfIds, StringIdGetters};
use crate::{ElfObject, ElfSectionContent};

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
    type StringId = String;
}

impl<F> ConvertibleElfIds<F> for StringIds
where
    F: ElfIds,
    F::StringId: StringIdGetters<F>,
{
    fn create_conversion_map(
        &mut self,
        object: &ElfObject<F>,
        string_ids: &[F::StringId],
    ) -> IdConversionMap<F, Self>
    where
        F: ElfIds,
        Self: Sized,
    {
        let mut map = IdConversionMap::<F, Self>::new();

        for string_id in string_ids {
            let ElfSectionContent::StringTable(ref table) = object
                .sections
                .get(string_id.section())
                .expect("missing string table")
                .content
            else {
                panic!("invalid type of string table")
            };
            let string = table
                .get(string_id.offset())
                .expect("missing string")
                .to_string();
            map.string_ids.insert(string_id.clone(), string);
        }

        for (i, (id, section)) in object.sections.iter().enumerate() {
            map.section_ids.insert(
                id.clone(),
                format!(
                    "{} {}",
                    format_number(i, object.sections.len()),
                    map.string_ids.get(&section.name).unwrap(),
                ),
            );

            match &section.content {
                ElfSectionContent::SymbolTable(table) => {
                    for (i, (id, symbol)) in table.symbols.iter().enumerate() {
                        let symbol_name = map.string_ids.get(&symbol.name).unwrap().to_string();
                        map.symbol_ids.insert(
                            id.clone(),
                            format!(
                                "{} {}",
                                format_number(i, table.symbols.len()),
                                if symbol_name.is_empty() {
                                    "<empty>"
                                } else {
                                    &symbol_name
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
