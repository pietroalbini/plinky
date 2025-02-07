use crate::passes::merge_sections::deduplicate::Deduplication;
use crate::repr::object::Object;
use crate::repr::relocations::{RelocationAddend, RelocationAddendError};
use crate::repr::sections::{SectionContent, SectionId};
use crate::repr::symbols::views::AllSymbols;
use crate::repr::symbols::{LoadSymbolsError, SymbolValue, UpcomingSymbol};
use plinky_macros::{Display, Error};
use plinky_utils::ints::Offset;
use std::collections::BTreeMap;

pub(super) fn run(
    object: &mut Object,
    deduplications: BTreeMap<SectionId, Deduplication>,
) -> Result<(), RewriteError> {
    // Some sections have relocations directly from the start of a deduplicated section, declaring
    // the offset into the deduplicated section as the relocation addend. Manually rewrite those.
    for section in object.sections.iter_mut() {
        let SectionContent::Data(data) = &mut section.content else { continue };
        for relocation in &mut data.relocations {
            let symbol = object.symbols.get(relocation.symbol);
            let SymbolValue::Section { section } = symbol.value() else { continue };

            if let Some(deduplication) = deduplications.get(&section) {
                let addend = relocation.addend(object.env.endian, &data.bytes)?;
                if let Some(new_offset) = deduplication.map.get(&addend) {
                    if let Some(symbol) = object.symbols.section_symbol_id(deduplication.target) {
                        relocation.symbol = symbol;
                    } else {
                        relocation.symbol = object
                            .symbols
                            .add(UpcomingSymbol::Section { section: deduplication.target })
                            .map_err(RewriteError::CreateSectionSymbol)?;
                    }
                    relocation.addend = RelocationAddend::Explicit(*new_offset);
                } else {
                    return Err(RewriteError::InvalidOffsetInDeduplicatedSection {
                        section,
                        offset: addend,
                    });
                }
            }
        }
    }

    // Update all symbols pointing to deduplicated sections to point to the new locations.
    let mut symbols_to_remove = Vec::new();
    for symbol in object.symbols.iter_mut(&AllSymbols) {
        let (section, offset) = match symbol.value() {
            SymbolValue::Section { section } => {
                if deduplications.contains_key(&section) {
                    symbols_to_remove.push(symbol.id());
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
            | SymbolValue::Null => continue,
        };

        if let Some(deduplication) = deduplications.get(&section) {
            if let Some(new_offset) = deduplication.map.get(&offset) {
                symbol.set_value(SymbolValue::SectionRelative {
                    section: deduplication.target,
                    offset: *new_offset,
                });
            } else {
                return Err(RewriteError::InvalidOffsetInDeduplicatedSection { section, offset });
            }
        }
    }
    for symbol_id in symbols_to_remove {
        object.symbols.remove(symbol_id);
    }

    Ok(())
}

#[derive(Debug, Error, Display)]
pub(crate) enum RewriteError {
    #[display(
        "something points to offset {offset} of section {section:?}, \
         which is not a merge boundary"
    )]
    InvalidOffsetInDeduplicatedSection { section: SectionId, offset: Offset },
    #[display("failed to retrieve the relocation addend")]
    RelocationAddend(#[from] RelocationAddendError),
    #[display("failed to create the section symbol")]
    CreateSectionSymbol(#[source] LoadSymbolsError),
}
