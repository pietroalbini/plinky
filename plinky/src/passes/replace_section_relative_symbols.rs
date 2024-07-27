use crate::passes::layout::Layout;
use crate::repr::object::Object;
use crate::repr::symbols::views::AllSymbols;
use crate::repr::symbols::{ResolveSymbolError, ResolvedSymbol, SymbolValue};
use plinky_macros::{Display, Error};

pub(crate) fn replace(
    object: &mut Object,
    layout: &Layout,
) -> Result<(), ReplaceSectionRelativeSymbolsError> {
    for (_, symbol) in object.symbols.iter_mut(&AllSymbols) {
        let SymbolValue::SectionRelative { .. } = symbol.value() else {
            continue;
        };

        let resolved = symbol.resolve(layout, 0.into())?;
        // Note that the section returned by symbol resolution might be different than the section
        // of the symbol itself. This could happen due to deduplication, as the section the
        // original symbol points to might be a deduplication facade.
        let ResolvedSymbol::Address { section, memory_address } = resolved else {
            panic!("section relative address doesn't resolve into an address");
        };

        symbol.set_value(SymbolValue::SectionVirtualAddress { section, memory_address });
    }

    Ok(())
}

#[derive(Debug, Display, Error)]
#[display("failed to replace addresses of section relative symbols")]
pub(crate) enum ReplaceSectionRelativeSymbolsError {
    #[transparent]
    ResolveSymbol(ResolveSymbolError),
}
