use crate::interner::Interned;
use crate::passes::layout::{AddressResolutionError, Layout};
use crate::utils::ints::{Absolute, Address, Offset, OutOfBoundsError};
use plinky_diagnostics::ObjectSpan;
use plinky_elf::ids::serial::{SectionId, SymbolId};
use plinky_macros::{Display, Error};

#[derive(Debug)]
pub(crate) struct Symbol {
    pub(crate) id: SymbolId,
    pub(crate) name: Interned<String>,
    pub(crate) type_: SymbolType,
    pub(crate) stt_file: Option<Interned<String>>,
    pub(crate) span: Interned<ObjectSpan>,
    pub(crate) visibility: SymbolVisibility,
    pub(crate) value: SymbolValue,
    pub(crate) needed_by_dynamic: bool,
    pub(crate) exclude_from_tables: bool,
}

impl Symbol {
    pub(crate) fn resolve(
        &self,
        layout: &Layout,
        offset: Offset,
    ) -> Result<ResolvedSymbol, ResolveSymbolError> {
        fn resolve_inner(
            symbol: &Symbol,
            layout: &Layout,
            offset: Offset,
        ) -> Result<ResolvedSymbol, ResolveSymbolErrorKind> {
            match &symbol.value {
                SymbolValue::Undefined => Err(ResolveSymbolErrorKind::Undefined),
                SymbolValue::Absolute { value } => {
                    assert!(offset == Offset::from(0));
                    Ok(ResolvedSymbol::Absolute(*value))
                }
                SymbolValue::SectionRelative { section, offset: section_offset } => {
                    match layout.address(*section, section_offset.add(offset)?) {
                        Ok((section, memory_address)) => {
                            Ok(ResolvedSymbol::Address { section, memory_address })
                        }
                        Err(err) => Err(ResolveSymbolErrorKind::Layout(err)),
                    }
                }
                SymbolValue::SectionVirtualAddress { section, memory_address } => {
                    Ok(ResolvedSymbol::Address {
                        section: *section,
                        memory_address: memory_address.offset(offset)?,
                    })
                }
                SymbolValue::Null => Err(ResolveSymbolErrorKind::Null),
            }
        }

        resolve_inner(self, layout, offset)
            .map_err(|inner| ResolveSymbolError { symbol: self.name, inner })
    }
}

#[derive(Debug)]
pub(crate) enum SymbolType {
    NoType,
    Function,
    Object,
    Section,
}

#[derive(Debug)]
pub(crate) enum SymbolVisibility {
    Local,
    Global { weak: bool, hidden: bool },
}

#[derive(Debug)]
pub(crate) enum SymbolValue {
    Absolute { value: Absolute },
    SectionRelative { section: SectionId, offset: Offset },
    SectionVirtualAddress { section: SectionId, memory_address: Address },
    Undefined,
    Null,
}

pub(crate) enum ResolvedSymbol {
    Absolute(Absolute),
    Address { section: SectionId, memory_address: Address },
}

#[derive(Debug, Error, Display)]
#[display("failed to resolve symbol {symbol}")]
pub(crate) struct ResolveSymbolError {
    pub(crate) symbol: Interned<String>,
    #[source]
    pub(crate) inner: ResolveSymbolErrorKind,
}

#[derive(Debug, Error, Display)]
pub(crate) enum ResolveSymbolErrorKind {
    #[display("the symbol is the null symbol")]
    Null,
    #[display("symbol is not defined")]
    Undefined,
    #[transparent]
    Layout(AddressResolutionError),
    #[transparent]
    OutOfBounds(OutOfBoundsError),
}
