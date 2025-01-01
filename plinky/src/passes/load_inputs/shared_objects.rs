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

    let name = match dynamic_reader.soname().map_err(SharedObjectError::ReadSoname)? {
        Some(soname) => intern(soname),
        None => match library_name {
            LibraryName::Known(path) => intern(path),
            LibraryName::InsideArchive => todo!(),
        },
    };

    let symbols = dynamic_reader.symbols().map_err(SharedObjectError::ReadSymbols)?;

    if options.as_needed && !is_needed(symbols, object, name) {
        return Ok(());
    }

    let span = intern(span);
    let mut undefined_global_symbols = Vec::new();
    for symbol in dynamic_reader.symbols().map_err(SharedObjectError::ReadSymbols)? {
        if symbol.defined {
            object
                .symbols
                .add(UpcomingSymbol::ExternallyDefined {
                    name: intern(&symbol.name),
                    span,
                    visibility: symbol.visibility,
                    binding: symbol.binding.clone(),
                })
                .map_err(SharedObjectError::AddSymbol)?;
        } else if let ElfSymbolBinding::Global = symbol.binding {
            undefined_global_symbols.push(intern(&symbol.name));
        }
    }

    object.inputs.push(Input {
        span,
        shared_object: Some(InputSharedObject {
            name,
            dependencies: dynamic_reader
                .needed_libraries()
                .map_err(SharedObjectError::NeededFailed)?
                .iter()
                .map(intern)
                .collect(),
            undefined_global_symbols,
        }),
        gnu_properties: GnuProperties { x86_isa_used: None, x86_features_2_used: None },
    });

    Ok(())
}

fn is_needed(
    dynamic_symbols: &[ElfSymbolInDynamic],
    object: &Object,
    name: Interned<String>,
) -> bool {
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

    // Then check if any library that doesn't depend on the current one has an undefined symbol
    // provided by this library.
    for input in &object.inputs {
        let Some(input_shared) = &input.shared_object else { continue };

        // The dependency already depends on the shared object being loaded, so there is no need to
        // mark it as a dependency of the final output.
        if input_shared.dependencies.contains(&name) {
            continue;
        }

        for symbol in &input_shared.undefined_global_symbols {
            if provided_symbols.contains(&symbol) {
                return true;
            }
        }
    }

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
    #[display("reading the dependencies of the shared library failed")]
    NeededFailed(#[source] ReadDynamicError),
}
