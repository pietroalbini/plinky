use crate::cli::CliInputOptions;
use crate::interner::{intern, Interned};
use crate::passes::load_inputs::read_objects::LibraryName;
use crate::repr::object::{GnuProperties, Input, InputSharedObject, Object};
use crate::repr::symbols::views::AllSymbols;
use crate::repr::symbols::{LoadSymbolsError, SymbolValue, SymbolVisibility, UpcomingSymbol};
use plinky_diagnostics::ObjectSpan;
use plinky_elf::{ElfReader, ElfSymbolBinding, ElfSymbolInDynamic, ReadDynamicError};
use plinky_macros::{Display, Error};
use std::collections::HashSet;

pub(super) fn load_shared_object(
    object: &mut Object,
    reader: &mut ElfReader<'_>,
    library_name: &LibraryName,
    options: CliInputOptions,
    span: ObjectSpan,
) -> Result<(), SharedObjectError> {
    let mut dynamic_reader = reader.dynamic().map_err(SharedObjectError::ReadSegment)?;
    let symbols = dynamic_reader.symbols().map_err(SharedObjectError::ReadSymbols)?;

    if options.as_needed && !is_needed(symbols, object) {
        return Ok(());
    }

    let span = intern(span);
    for symbol in dynamic_reader.symbols().map_err(SharedObjectError::ReadSymbols)? {
        if !symbol.defined {
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
        None => match library_name {
            LibraryName::Known(path) => intern(path),
            LibraryName::InsideArchive => todo!(),
        },
    };

    object.inputs.push(Input {
        span,
        shared_object: Some(InputSharedObject { name }),
        gnu_properties: GnuProperties { x86_isa_used: None, x86_features_2_used: None },
    });

    Ok(())
}

fn is_needed(dynamic_symbols: &[ElfSymbolInDynamic], object: &Object) -> bool {
    let provided_symbols = dynamic_symbols
        .iter()
        .filter(|s| matches!(s.binding, ElfSymbolBinding::Global | ElfSymbolBinding::Weak))
        .filter(|s| s.defined)
        .map(|symbol| intern(&symbol.name))
        .collect::<HashSet<Interned<String>>>();

    // First, check if any global non-weak undefined symbol is provided by the shared library.
    for sym in object.symbols.iter(&AllSymbols) {
        let SymbolValue::Undefined = &sym.value() else { continue };
        let SymbolVisibility::Global { weak: false, .. } = sym.visibility() else { continue };

        if provided_symbols.contains(&sym.name()) {
            return true;
        }
    }

    // TODO: check if any library that doesn't need the current one has an undefined symbol
    // provided by this library.

    false
}

#[derive(Debug, Error, Display)]
pub(crate) enum SharedObjectError {
    #[display("parsing the dynamic segment failed")]
    ReadSegment(#[source] ReadDynamicError),
    #[display("reading the symbol names failed")]
    ReadSymbols(#[source] ReadDynamicError),
    #[display("failed to add symbol from the dynamic library")]
    AddSymbol(#[source] LoadSymbolsError),
    #[display("reading the shared object name failed")]
    ReadSoname(#[source] ReadDynamicError),
}
