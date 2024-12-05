use crate::interner::intern;
use crate::repr::object::{GnuProperties, Input, InputSharedObject, Object};
use crate::repr::symbols::{LoadSymbolsError, UpcomingSymbol};
use plinky_diagnostics::ObjectSpan;
use plinky_elf::{ElfReader, ReadDynamicError};
use plinky_macros::{Display, Error};

pub(super) fn load_shared_object(
    object: &mut Object,
    reader: &mut ElfReader<'_>,
    file_name: &str,
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

    let name = match dynamic_reader.soname().map_err(SharedObjectError::ReadSoname)? {
        Some(soname) => intern(soname),
        None => {
            // FIXME: once -lfoo is implemented, this should return the base file name being loaded.
            // https://maskray.me/blog/2020-11-15-explain-gnu-linker-options#sonamename
            intern(file_name)
        }
    };

    object.inputs.push(Input {
        span,
        shared_object: Some(InputSharedObject { name }),
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
    #[display("reading the shared object name failed")]
    ReadSoname(#[source] ReadDynamicError),
}
