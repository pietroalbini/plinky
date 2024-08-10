use crate::interner::{intern, Interned};
use crate::repr::symbols::LoadSymbolsError;
use crate::utils::address_resolver::{AddressResolutionError, AddressResolver};
use plinky_utils::ints::{Absolute, Address, Offset, OutOfBoundsError};
use plinky_diagnostics::ObjectSpan;
use plinky_elf::ids::serial::{SectionId, SerialIds, SymbolId};
use plinky_elf::{
    ElfSymbol, ElfSymbolBinding, ElfSymbolDefinition, ElfSymbolType, ElfSymbolVisibility,
};
use plinky_macros::{Display, Error, Getters};

#[derive(Debug, Getters)]
pub(crate) struct Symbol {
    #[get]
    id: SymbolId,
    #[get]
    name: Interned<String>,
    #[get]
    type_: SymbolType,
    #[get]
    stt_file: Option<Interned<String>>,
    #[get]
    span: Interned<ObjectSpan>,
    #[get]
    visibility: SymbolVisibility,
    #[get]
    value: SymbolValue,
    #[get]
    needed_by_dynamic: bool,
    #[get]
    exclude_from_tables: bool,
}

impl Symbol {
    pub(crate) fn new_null(id: SymbolId) -> Self {
        Self {
            id,
            name: intern(""),
            type_: SymbolType::NoType,
            stt_file: None,
            span: intern(ObjectSpan::new_synthetic()),
            visibility: SymbolVisibility::Local,
            value: SymbolValue::Null,
            needed_by_dynamic: false,
            exclude_from_tables: false,
        }
    }

    pub(crate) fn new_global_unknown(id: SymbolId, name: &str) -> Self {
        Self {
            id,
            name: intern(name),
            type_: SymbolType::NoType,
            stt_file: None,
            span: intern(ObjectSpan::new_synthetic()),
            visibility: SymbolVisibility::Global { weak: false, hidden: false },
            value: SymbolValue::Undefined,
            needed_by_dynamic: false,
            exclude_from_tables: false,
        }
    }

    pub(crate) fn new_elf(
        id: SymbolId,
        elf: ElfSymbol<SerialIds>,
        name: Interned<String>,
        span: Interned<ObjectSpan>,
        stt_file: Option<Interned<String>>,
    ) -> Result<Self, LoadSymbolsError> {
        let type_ = match elf.type_ {
            ElfSymbolType::NoType => SymbolType::NoType,
            ElfSymbolType::Object => SymbolType::Object,
            ElfSymbolType::Function => SymbolType::Function,
            ElfSymbolType::Section => SymbolType::Section,
            ElfSymbolType::File => {
                return Err(LoadSymbolsError::UnsupportedFileSymbolType);
            }
            ElfSymbolType::Unknown(_) => {
                return Err(LoadSymbolsError::UnsupportedUnknownSymbolType);
            }
        };

        let hidden = match elf.visibility {
            ElfSymbolVisibility::Default => false,
            ElfSymbolVisibility::Hidden => true,
            other => return Err(LoadSymbolsError::UnsupportedVisibility(other)),
        };

        Ok(Symbol {
            id,
            name,
            type_,
            stt_file,
            span,
            visibility: match (elf.binding, hidden) {
                (ElfSymbolBinding::Local, false) => SymbolVisibility::Local,
                (ElfSymbolBinding::Local, true) => {
                    return Err(LoadSymbolsError::LocalHiddenSymbol);
                }
                (ElfSymbolBinding::Global, hidden) => {
                    SymbolVisibility::Global { weak: false, hidden }
                }
                (ElfSymbolBinding::Weak, hidden) => SymbolVisibility::Global { weak: true, hidden },
                (ElfSymbolBinding::Unknown(_), _) => {
                    return Err(LoadSymbolsError::UnsupportedUnknownSymbolBinding);
                }
            },
            value: match elf.definition {
                ElfSymbolDefinition::Undefined => SymbolValue::Undefined,
                ElfSymbolDefinition::Absolute => SymbolValue::Absolute { value: elf.value.into() },
                ElfSymbolDefinition::Common => todo!(),
                ElfSymbolDefinition::Section(section) => {
                    SymbolValue::SectionRelative { section, offset: (elf.value as i64).into() }
                }
            },
            needed_by_dynamic: false,
            exclude_from_tables: false,
        })
    }

    pub(super) fn set_id(&mut self, id: SymbolId) {
        self.id = id;
    }

    pub(crate) fn set_value(&mut self, value: SymbolValue) {
        self.value = value;
    }

    pub(crate) fn set_visibility(&mut self, visibility: SymbolVisibility) {
        self.visibility = visibility;
    }

    pub(crate) fn mark_needed_by_dynamic(&mut self) {
        self.needed_by_dynamic = true;
    }

    pub(crate) fn mark_exclude_from_tables(&mut self) {
        self.exclude_from_tables = true;
    }

    pub(crate) fn resolve(
        &self,
        resolver: &AddressResolver<'_>,
        offset: Offset,
    ) -> Result<ResolvedSymbol, ResolveSymbolError> {
        fn resolve_inner(
            symbol: &Symbol,
            resolver: &AddressResolver<'_>,
            offset: Offset,
        ) -> Result<ResolvedSymbol, ResolveSymbolErrorKind> {
            match &symbol.value {
                SymbolValue::Undefined => Err(ResolveSymbolErrorKind::Undefined),
                SymbolValue::Absolute { value } => {
                    assert!(offset == Offset::from(0));
                    Ok(ResolvedSymbol::Absolute(*value))
                }
                SymbolValue::SectionRelative { section, offset: section_offset } => {
                    match resolver.address(*section, section_offset.add(offset)?) {
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

        resolve_inner(self, resolver, offset)
            .map_err(|inner| ResolveSymbolError { symbol: self.name, inner })
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum SymbolType {
    NoType,
    Function,
    Object,
    Section,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum SymbolVisibility {
    Local,
    Global { weak: bool, hidden: bool },
}

#[derive(Debug, Clone, Copy)]
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
