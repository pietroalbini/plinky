use crate::interner::intern;
use crate::repr::object::{GnuProperties, Input, Object};
use crate::repr::symbols::{LoadSymbolsError, UpcomingSymbol};
use plinky_diagnostics::ObjectSpan;
use plinky_elf::{ElfReader, ReadDynamicError};
use plinky_macros::{Display, Error};

pub(super) fn load_shared_object(
    object: &mut Object,
    reader: &mut ElfReader<'_>,
    span: ObjectSpan,
) -> Result<(), SharedObjectError> {
    let mut dynamic_reader = reader.dynamic().map_err(SharedObjectError::ReadSegment)?;

    let span = intern(span);
    let mut first = true;
    for symbol in dynamic_reader.symbols().map_err(SharedObjectError::ReadSymbols)? {
        if first {
            first = false;
            if !symbol.name.is_empty() {
                return Err(SharedObjectError::FirstSymbolNotNull);
            }
            // Skip the null symbol.
            continue;
        }
        object
            .symbols
            .add(UpcomingSymbol::ExternallyDefined {
                name: intern(&symbol.name),
                span,
                visibility: symbol.visibility,
                binding: symbol.binding.clone(),
            })
            .map_err(SharedObjectError::AddSymbol)?;
    }

    object.inputs.push(Input {
        span,
        shared_object: true,
        gnu_properties: GnuProperties { x86_isa_used: None, x86_features_2_used: None },
    });

    Ok(())
}

#[derive(Debug, Error, Display)]
pub(crate) enum SharedObjectError {
    #[display("parsing the dynamic segment failed")]
    ReadSegment(#[source] ReadDynamicError),
    #[display("reading the symbol names failed")]
    ReadSymbols(#[source] ReadDynamicError),
    #[display("the first symbol is not the null symbol")]
    FirstSymbolNotNull,
    #[display("failed to add symbol from the dynamic library")]
    AddSymbol(#[source] LoadSymbolsError),
}
