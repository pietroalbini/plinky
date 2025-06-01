use crate::diagnostics::builders::UndefinedSymbolDiagnostic;
use crate::interner::{Interned, intern};
use crate::repr::sections::SectionId;
use crate::repr::symbols::{LoadSymbolsError, NULL_SYMBOL_ID, SymbolId};
use crate::utils::address_resolver::{AddressResolutionError, AddressResolver};
use plinky_diagnostics::ObjectSpan;
use plinky_elf::ids::ElfSectionId;
use plinky_elf::{
    ElfSymbol, ElfSymbolBinding, ElfSymbolDefinition, ElfSymbolType, ElfSymbolVisibility,
};
use plinky_macros::{Display, Error, Getters};
use plinky_utils::ints::{Absolute, Address, Offset, OutOfBoundsError};
use std::collections::{BTreeMap, BTreeSet};

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
    pub(crate) fn set_value(&mut self, value: SymbolValue) {
        // To ensure invariants are upheld, only some specific kinds of symbol value conversions
        // can happen. When adding a new allowed conversion, make sure to uphold that:
        //
        // - SymbolValue::Section must not be converted from or to.
        match (&self.value, &value) {
            (SymbolValue::SectionRelative { .. }, SymbolValue::SectionRelative { .. }) => {}
            (SymbolValue::SectionRelative { .. }, SymbolValue::SectionVirtualAddress { .. }) => {}
            (from, to) => panic!("cannot convert from {from:?} to {to:?}"),
        }

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
                SymbolValue::Undefined => {
                    Err(ResolveSymbolErrorKind::Undefined(UndefinedSymbolDiagnostic {
                        name: symbol.name(),
                        expected_visibility: symbol.visibility(),
                    }))
                }
                SymbolValue::Absolute { value } => {
                    assert!(offset == Offset::from(0));
                    Ok(ResolvedSymbol::Absolute(*value))
                }
                SymbolValue::Section { section } => match resolver.address(*section, offset) {
                    Ok((section, memory_address)) => {
                        Ok(ResolvedSymbol::Address { section, memory_address })
                    }
                    Err(err) => Err(ResolveSymbolErrorKind::Layout(err)),
                },
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
                SymbolValue::ExternallyDefined => Ok(ResolvedSymbol::ExternallyDefined),
                SymbolValue::SectionNotLoaded => Err(ResolveSymbolErrorKind::SectionNotLoaded),
                SymbolValue::Null => Err(ResolveSymbolErrorKind::Null),
            }
        }

        resolve_inner(self, resolver, offset)
            .map_err(|inner| ResolveSymbolError { symbol: self.name, inner })
    }

    pub(crate) fn is_null_symbol(&self) -> bool {
        self.id == NULL_SYMBOL_ID
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum SymbolType {
    NoType,
    Function,
    Object,
    Section,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SymbolVisibility {
    Local,
    Global { weak: bool, hidden: bool },
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum SymbolValue {
    Absolute { value: Absolute },
    Section { section: SectionId },
    SectionRelative { section: SectionId, offset: Offset },
    SectionVirtualAddress { section: SectionId, memory_address: Address },
    SectionNotLoaded,
    ExternallyDefined,
    Undefined,
    Null,
}

#[derive(Debug)]
pub(crate) enum UpcomingSymbol<'a> {
    Null,
    GlobalUnknown {
        name: Interned<String>,
    },
    GlobalHidden {
        name: Interned<String>,
        value: SymbolValue,
    },
    Section {
        section: SectionId,
        span: Interned<ObjectSpan>,
    },
    ExternallyDefined {
        name: Interned<String>,
        span: Interned<ObjectSpan>,
        visibility: ElfSymbolVisibility,
        binding: ElfSymbolBinding,
    },
    Elf {
        sections_not_loaded: &'a BTreeSet<ElfSectionId>,
        section_conversion: &'a BTreeMap<ElfSectionId, SectionId>,
        elf: ElfSymbol,
        resolved_name: Interned<String>,
        span: Interned<ObjectSpan>,
        stt_file: Option<Interned<String>>,
    },
}

impl UpcomingSymbol<'_> {
    pub(super) fn create(self, id: SymbolId) -> Result<Symbol, LoadSymbolsError> {
        let visibility = self.visibility()?;
        let name = self.name();
        let value = self.value();

        let mut type_ = SymbolType::NoType;
        let mut stt_file = None;
        let mut span = None;

        match &self {
            UpcomingSymbol::Null => {}
            UpcomingSymbol::GlobalUnknown { .. } => {}
            UpcomingSymbol::GlobalHidden { .. } => {}
            UpcomingSymbol::ExternallyDefined { span: new_span, .. } => span = Some(*new_span),
            UpcomingSymbol::Section { span: new_span, .. } => {
                type_ = SymbolType::Section;
                span = Some(*new_span);
            }
            UpcomingSymbol::Elf { elf, span: new_span, stt_file: new_stt_file, .. } => {
                type_ = match elf.type_ {
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
                stt_file = new_stt_file.clone();
                span = Some(*new_span);
            }
        };

        Ok(Symbol {
            id,
            name,
            type_,
            stt_file,
            span: span.unwrap_or_else(|| intern(ObjectSpan::new_synthetic())),
            visibility,
            value,
            needed_by_dynamic: false,
            exclude_from_tables: false,
        })
    }

    pub(super) fn name(&self) -> Interned<String> {
        match self {
            UpcomingSymbol::Null => intern(""),
            UpcomingSymbol::GlobalUnknown { name } => *name,
            UpcomingSymbol::GlobalHidden { name, .. } => *name,
            UpcomingSymbol::Section { .. } => intern(""),
            UpcomingSymbol::ExternallyDefined { name, .. } => *name,
            UpcomingSymbol::Elf { resolved_name, .. } => *resolved_name,
        }
    }

    pub(super) fn visibility(&self) -> Result<SymbolVisibility, LoadSymbolsError> {
        Ok(match self {
            UpcomingSymbol::Null => SymbolVisibility::Local,
            UpcomingSymbol::Section { .. } => SymbolVisibility::Local,
            UpcomingSymbol::GlobalUnknown { .. } => {
                SymbolVisibility::Global { weak: false, hidden: false }
            }
            UpcomingSymbol::GlobalHidden { .. } => {
                SymbolVisibility::Global { weak: false, hidden: true }
            }
            UpcomingSymbol::Elf { elf: ElfSymbol { visibility, binding, .. }, .. }
            | UpcomingSymbol::ExternallyDefined { visibility, binding, .. } => {
                let hidden = match visibility {
                    ElfSymbolVisibility::Default => false,
                    ElfSymbolVisibility::Hidden => true,
                    other => return Err(LoadSymbolsError::UnsupportedVisibility(*other)),
                };

                match (binding, hidden) {
                    (ElfSymbolBinding::Local, false) => SymbolVisibility::Local,
                    (ElfSymbolBinding::Local, true) => {
                        return Err(LoadSymbolsError::LocalHiddenSymbol);
                    }
                    (ElfSymbolBinding::Global, hidden) => {
                        SymbolVisibility::Global { weak: false, hidden }
                    }
                    (ElfSymbolBinding::Weak, hidden) => {
                        SymbolVisibility::Global { weak: true, hidden }
                    }
                    (ElfSymbolBinding::Unknown(_), _) => {
                        return Err(LoadSymbolsError::UnsupportedUnknownSymbolBinding);
                    }
                }
            }
        })
    }

    pub(super) fn value(&self) -> SymbolValue {
        match self {
            UpcomingSymbol::Null => SymbolValue::Null,
            UpcomingSymbol::GlobalUnknown { .. } => SymbolValue::Undefined,
            UpcomingSymbol::GlobalHidden { value, .. } => *value,
            UpcomingSymbol::Section { section, .. } => SymbolValue::Section { section: *section },
            UpcomingSymbol::ExternallyDefined { .. } => SymbolValue::ExternallyDefined,
            UpcomingSymbol::Elf {
                section_conversion,
                sections_not_loaded,
                resolved_name,
                elf,
                ..
            } => match elf.definition {
                ElfSymbolDefinition::Undefined => SymbolValue::Undefined,
                ElfSymbolDefinition::Absolute => SymbolValue::Absolute { value: elf.value.into() },
                ElfSymbolDefinition::Common => todo!(),
                ElfSymbolDefinition::Section(section) => {
                    if sections_not_loaded.contains(&section) {
                        SymbolValue::SectionNotLoaded
                    } else if *resolved_name == intern("") && elf.value == 0 {
                        SymbolValue::Section {
                            section: *section_conversion
                                .get(&section)
                                .expect("missing section conversion"),
                        }
                    } else {
                        SymbolValue::SectionRelative {
                            section: *section_conversion
                                .get(&section)
                                .expect("missing section conversion"),
                            offset: (elf.value as i64).into(),
                        }
                    }
                }
            },
        }
    }
}

pub(crate) enum ResolvedSymbol {
    ExternallyDefined,
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
    Undefined(#[diagnostic] UndefinedSymbolDiagnostic),
    #[display("the symbol points to a section that was not loaded")]
    SectionNotLoaded,
    #[transparent]
    Layout(AddressResolutionError),
    #[transparent]
    OutOfBounds(OutOfBoundsError),
}
