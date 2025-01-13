use crate::passes::merge_sections::deduplicate::Deduplication;
use crate::repr::object::Object;
use crate::repr::sections::SectionId;
use crate::repr::symbols::views::AllSymbols;
use crate::repr::symbols::{SymbolId, SymbolValue};
use plinky_macros::{Display, Error};
use plinky_utils::ints::Offset;
use std::collections::BTreeMap;

pub(super) fn run(
    object: &mut Object,
    deduplications: BTreeMap<SectionId, Deduplication>,
) -> Result<(), RewriteError> {
    for symbol in object.symbols.iter_mut(&AllSymbols) {
        let (section, offset) = match symbol.value() {
            SymbolValue::Section { section } => {
                if deduplications.contains_key(&section) {
                    symbol.set_value(SymbolValue::Poison);
                }
                continue;
            }
            SymbolValue::SectionRelative { section, offset } => (section, offset),
            SymbolValue::SectionVirtualAddress { .. } => {
                unreachable!("symbol should not have a SectionVirtualAddress value at this point")
            }
            SymbolValue::Absolute { .. }
            | SymbolValue::SectionNotLoaded
            | SymbolValue::ExternallyDefined
            | SymbolValue::Undefined
            | SymbolValue::Null
            | SymbolValue::Poison => continue,
        };

        if let Some(deduplication) = deduplications.get(&section) {
            if let Some(new_offset) = deduplication.map.get(&offset) {
                symbol.set_value(SymbolValue::SectionRelative {
                    section: deduplication.target,
                    offset: *new_offset,
                });
            } else {
                return Err(RewriteError::InvalidOffsetInDeduplicatedSection {
                    section,
                    symbol: symbol.id(),
                    offset,
                });
            }
        }
    }

    Ok(())
}

#[derive(Debug, Error, Display)]
pub(crate) enum RewriteError {
    #[display(
        "symbol {symbol:?} points to offset {offset} of section {section:?}, \
         which is not a merge boundary"
    )]
    InvalidOffsetInDeduplicatedSection { section: SectionId, symbol: SymbolId, offset: Offset },
}
