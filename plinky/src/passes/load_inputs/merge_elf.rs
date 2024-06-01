use crate::interner::{intern, Interned};
use crate::passes::load_inputs::section_groups::{SectionGroupsError, SectionGroupsForObject};
use crate::passes::load_inputs::strings::{MissingStringError, Strings};
use crate::repr::object::Object;
use crate::repr::relocations::UnsupportedRelocationType;
use crate::repr::sections::{DataSection, Section, SectionContent, UninitializedSection};
use crate::repr::symbols::{
    LoadSymbolsError, Symbol, SymbolType, SymbolValue, SymbolVisibility, Symbols,
};
use plinky_diagnostics::ObjectSpan;
use plinky_elf::ids::serial::{SectionId, SerialIds};
use plinky_elf::{
    ElfNote, ElfObject, ElfSectionContent, ElfSymbolBinding, ElfSymbolDefinition, ElfSymbolTable,
    ElfSymbolType, ElfSymbolVisibility,
};
use plinky_macros::{Display, Error};
use std::collections::BTreeMap;

pub(super) fn merge(
    object: &mut Object,
    strings: &mut Strings,
    mut section_groups: SectionGroupsForObject<'_>,
    source: ObjectSpan,
    elf: ElfObject<SerialIds>,
) -> Result<(), MergeElfError> {
    let mut symbol_tables = Vec::new();
    let mut program_sections = Vec::new();
    let mut uninitialized_sections = Vec::new();
    let mut pending_groups = Vec::new();
    let mut relocations = BTreeMap::new();

    for (section_id, section) in elf.sections.into_iter() {
        match section.content {
            ElfSectionContent::Null => {}
            ElfSectionContent::Program(program) => {
                program_sections.push((section_id, section.name, program))
            }
            ElfSectionContent::Uninitialized(uninit) => {
                uninitialized_sections.push((section_id, section.name, uninit));
            }
            ElfSectionContent::SymbolTable(table) => symbol_tables.push((section.name, table)),
            ElfSectionContent::StringTable(table) => strings.load_table(section_id, table),
            ElfSectionContent::RelocationsTable(table) => {
                relocations.insert(table.applies_to_section, table.relocations);
            }
            ElfSectionContent::Group(group) => pending_groups.push((section_id, group)),
            ElfSectionContent::Note(table) => {
                for note in table.notes {
                    match note {
                        ElfNote::Unknown(unknown) => {
                            return Err(MergeElfError::UnsupportedUnknownNote {
                                name: unknown.name,
                                type_: unknown.type_,
                            })
                        }
                    }
                }
            }
            ElfSectionContent::Unknown(unknown) => {
                return Err(MergeElfError::UnsupportedUnknownSection { id: unknown.id });
            }
        }
    }

    // This is loaded after the string tables are loaded by the previous iteration, as we need to
    // resolve the signature of section groups.
    for (id, group) in pending_groups {
        section_groups.add_group(&strings, &symbol_tables, id, group)?;
    }

    // This is loaded after the string tables are loaded by the previous iteration, as we need
    // to resolve the strings as part of symbol loading.
    for (name_id, mut table) in symbol_tables {
        section_groups.filter_symbol_table(&mut table)?;
        merge_symbols(&mut object.symbols, intern(source.clone()), table, &strings).map_err(
            |inner| MergeElfError::SymbolsLoadingFailed {
                section_name: strings.get(name_id).unwrap_or("<unknown>").into(),
                inner,
            },
        )?;
    }

    for (id, name, uninit) in uninitialized_sections {
        if section_groups.should_skip_section(id) {
            continue;
        }
        object.sections.add(Section {
            id,
            name: intern(
                strings.get(name).map_err(|err| MergeElfError::MissingSectionName { id, err })?,
            ),
            perms: uninit.perms,
            source: source.clone(),
            content: SectionContent::Uninitialized(UninitializedSection { len: uninit.len }),
        });
    }

    for (id, name, program) in program_sections {
        if section_groups.should_skip_section(id) {
            continue;
        }
        object.sections.add(Section {
            id,
            name: intern(
                strings.get(name).map_err(|err| MergeElfError::MissingSectionName { id, err })?,
            ),
            perms: program.perms,
            source: source.clone(),
            content: SectionContent::Data(DataSection {
                deduplication: program.deduplication,
                bytes: program.raw.0,
                relocations: relocations
                    .remove(&id)
                    .unwrap_or_default()
                    .into_iter()
                    .map(|r| r.try_into())
                    .collect::<Result<_, _>>()?,
            }),
        });
    }
    Ok(())
}

fn merge_symbols(
    symbols: &mut Symbols,
    span: Interned<ObjectSpan>,
    table: ElfSymbolTable<SerialIds>,
    strings: &Strings,
) -> Result<(), LoadSymbolsError> {
    let mut stt_file = None;
    let mut is_first = true;
    for (symbol_id, elf_symbol) in table.symbols.into_iter() {
        let name: Interned<String> = intern(
            strings
                .get(elf_symbol.name)
                .map_err(|_| LoadSymbolsError::MissingSymbolName(symbol_id))?,
        );

        if is_first {
            is_first = false;

            // Instead of creating the null symbol for every object we load, we instead
            // redirect it to the shared null symbol defined during initialization.
            if name.resolve().is_empty()
                && matches!(elf_symbol.definition, ElfSymbolDefinition::Undefined)
                && matches!(elf_symbol.type_, ElfSymbolType::NoType)
            {
                symbols.add_redirect(symbol_id, symbols.null_symbol_id());
                continue;
            }
        }

        let type_ = match elf_symbol.type_ {
            ElfSymbolType::NoType => SymbolType::NoType,
            ElfSymbolType::Object => SymbolType::Object,
            ElfSymbolType::Function => SymbolType::Function,
            ElfSymbolType::Section => SymbolType::Section,
            // The file symbol type is not actually used, so we can omit it.
            ElfSymbolType::File => {
                stt_file = Some(name);
                continue;
            }
            ElfSymbolType::Unknown(_) => {
                return Err(LoadSymbolsError::UnsupportedUnknownSymbolType)
            }
        };

        let hidden = match elf_symbol.visibility {
            ElfSymbolVisibility::Default => false,
            ElfSymbolVisibility::Hidden => true,
            other => return Err(LoadSymbolsError::UnsupportedVisibility(other)),
        };

        let symbol = Symbol {
            id: symbol_id,
            name,
            type_,
            stt_file,
            span,
            visibility: match (elf_symbol.binding, hidden) {
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
            value: match elf_symbol.definition {
                ElfSymbolDefinition::Undefined => SymbolValue::Undefined,
                ElfSymbolDefinition::Absolute => {
                    SymbolValue::Absolute { value: elf_symbol.value.into() }
                }
                ElfSymbolDefinition::Common => todo!(),
                ElfSymbolDefinition::Section(section) => SymbolValue::SectionRelative {
                    section,
                    offset: (elf_symbol.value as i64).into(),
                },
            },
        };

        symbols.add_symbol(symbol)?;
    }
    Ok(())
}

#[derive(Debug, Error, Display)]
pub(crate) enum MergeElfError {
    #[display("unsupported note with name {name} and type {type_}")]
    UnsupportedUnknownNote { name: String, type_: u32 },
    #[display("unknown section with type {id:#x?} is not supported")]
    UnsupportedUnknownSection { id: u32 },
    #[transparent]
    UnsupportedRelocation(UnsupportedRelocationType),
    #[display("failed to load symbols from section {section_name}")]
    SymbolsLoadingFailed {
        section_name: String,
        #[source]
        inner: LoadSymbolsError,
    },
    #[display("failed to fetch section name for section {id:?}")]
    MissingSectionName {
        id: SectionId,
        #[source]
        err: MissingStringError,
    },
    #[transparent]
    SectionGroups(SectionGroupsError),
}
