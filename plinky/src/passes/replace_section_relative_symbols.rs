use crate::passes::layout::Layout;
use crate::repr::object::Object;
use crate::repr::symbols::{ResolveSymbolError, ResolvedSymbol, SymbolValue};
use plinky_macros::{Display, Error};

pub(crate) fn replace(
    object: &mut Object,
    layout: &Layout,
) -> Result<(), ReplaceSectionRelativeSymbolsError> {
    for (_, symbol) in object.symbols.iter_mut() {
        let SymbolValue::SectionRelative { section, .. } = symbol.value else {
            continue;
        };

        let resolved = symbol.resolve(layout, 0)?;
        let ResolvedSymbol::Address(memory_address) = resolved else {
            panic!("section relative address doesn't resolve into an address");
        };

        symbol.value = SymbolValue::SectionVirtualAddress { section, memory_address };
    }

    Ok(())
}

#[derive(Debug, Display, Error)]
#[display("failed to replace addresses of section relative symbols")]
pub(crate) struct ReplaceSectionRelativeSymbolsError {
    #[from]
    inner: ResolveSymbolError,
}
