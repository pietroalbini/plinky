use crate::interner::{Interned, intern};
use crate::passes::load_inputs::section_groups::{SectionGroupsError, SectionGroupsForObject};
use crate::passes::load_inputs::strings::{MissingStringError, Strings};
use crate::repr::object::{GnuProperties, Input, Object};
use crate::repr::relocations::{Relocation, UnsupportedRelocationType};
use crate::repr::sections::{DataSection, SectionId, UninitializedSection};
use crate::repr::symbols::{LoadSymbolsError, SymbolId, Symbols, UpcomingSymbol};
use plinky_diagnostics::ObjectSpan;
use plinky_elf::ids::{ElfSectionId, ElfSymbolId};
use plinky_elf::{
    ElfGnuProperty, ElfNote, ElfObject, ElfRel, ElfRela, ElfSectionContent, ElfSymbolDefinition,
    ElfSymbolTable, ElfSymbolType,
};
use plinky_macros::{Display, Error};
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn merge_elf(
    object: &mut Object,
    strings: &mut Strings,
    mut section_groups: SectionGroupsForObject<'_>,
    elf: ElfObject,
    source: ObjectSpan,
) -> Result<(), MergeElfError> {
    let mut symbol_tables = Vec::new();
    let mut program_sections = Vec::new();
    let mut uninitialized_sections = Vec::new();
    let mut pending_groups = Vec::new();
    let mut relocations = BTreeMap::new();
    let mut section_placeholders = BTreeMap::new();
    let mut symbol_conversion = BTreeMap::new();

    let mut x86_isa_used = None;
    let mut x86_features_2_used = None;

    let mut all_elf_section_ids = Vec::new();
    for (section_id, section) in elf.sections.into_iter() {
        all_elf_section_ids.push(section_id);
        match section.content {
            ElfSectionContent::Null => {}
            ElfSectionContent::Program(program) => {
                section_placeholders.insert(section_id, object.sections.reserve_placeholder());
                program_sections.push((section_id, section.name, section.retain, program))
            }
            ElfSectionContent::Uninitialized(uninit) => {
                section_placeholders.insert(section_id, object.sections.reserve_placeholder());
                uninitialized_sections.push((section_id, section.name, section.retain, uninit));
            }
            ElfSectionContent::SymbolTable(table) => {
                if table.dynsym {
                    return Err(MergeElfError::UnsupportedDynamicSymbolTable);
                }
                symbol_tables.push((section.name, table))
            }
            ElfSectionContent::StringTable(table) => strings.load_table(section_id, table),
            ElfSectionContent::Rel(table) => {
                relocations.insert(
                    table.applies_to_section,
                    table.relocations.into_iter().map(EitherRelocation::Rel).collect::<Vec<_>>(),
                );
            }
            ElfSectionContent::Rela(table) => {
                relocations.insert(
                    table.applies_to_section,
                    table.relocations.into_iter().map(EitherRelocation::Rela).collect::<Vec<_>>(),
                );
            }
            ElfSectionContent::Group(group) => pending_groups.push((section_id, group)),
            ElfSectionContent::Hash(_) | ElfSectionContent::GnuHash(_) => {
                // We don't need hash tables imported from the ELF file, we build our own.
            }
            ElfSectionContent::Dynamic(_) => {
                return Err(MergeElfError::UnsupportedDynamicSection);
            }
            ElfSectionContent::Note(table) => {
                for note in table.notes {
                    match note {
                        ElfNote::GnuProperties(properties) => {
                            for property in properties {
                                match property {
                                    ElfGnuProperty::X86Features2Used(val) => {
                                        if x86_features_2_used.is_some() {
                                            return Err(MergeElfError::DuplicateGnuProperty);
                                        }
                                        x86_features_2_used = Some(val);
                                    }
                                    ElfGnuProperty::X86IsaUsed(val) => {
                                        if x86_isa_used.is_some() {
                                            return Err(MergeElfError::DuplicateGnuProperty);
                                        }
                                        x86_isa_used = Some(val)
                                    }
                                    ElfGnuProperty::Unknown(unknown) => {
                                        return Err(MergeElfError::UnsupportedUnknownGnuProperty {
                                            type_: unknown.type_,
                                        });
                                    }
                                }
                            }
                        }
                        ElfNote::Unknown(unknown) => {
                            return Err(MergeElfError::UnsupportedUnknownNote {
                                name: unknown.name,
                                type_: unknown.type_,
                            });
                        }
                    }
                }
            }
            ElfSectionContent::Unknown(unknown) => {
                return Err(MergeElfError::UnsupportedUnknownSection { id: unknown.id });
            }
        }
    }

    let sections_not_loaded = all_elf_section_ids
        .into_iter()
        .filter(|elf_id| !section_placeholders.contains_key(elf_id))
        .collect::<BTreeSet<_>>();

    // This is loaded after the string tables are loaded by the previous iteration, as we need to
    // resolve the signature of section groups.
    for (id, group) in pending_groups {
        section_groups.add_group(&strings, &symbol_tables, id, group)?;
    }

    // This is loaded after the string tables are loaded by the previous iteration, as we need
    // to resolve the strings as part of symbol loading.
    for (name_id, mut table) in symbol_tables {
        section_groups.filter_symbol_table(&mut table)?;

        let table_name = strings.get(name_id).unwrap_or("<unknown>").to_string();
        merge_symbols(
            &mut object.symbols,
            &section_groups,
            &section_placeholders,
            &sections_not_loaded,
            &mut symbol_conversion,
            intern(source.clone()),
            table,
            &strings,
            &table_name,
        )?;
    }

    for (id, name, retain, uninit) in uninitialized_sections {
        if section_groups.should_skip_section(id) {
            continue;
        }

        let placeholder = *section_placeholders.get(&id).expect("missing placeholder");
        object
            .sections
            .builder(
                strings.get(name).map_err(|err| MergeElfError::MissingSectionName { id, err })?,
                UninitializedSection { perms: uninit.perms, len: uninit.len.into() },
            )
            .source(source.clone())
            .retain(retain)
            .create_in_placeholder(placeholder);
    }

    for (id, name, retain, program) in program_sections {
        if section_groups.should_skip_section(id) {
            continue;
        }
        let placeholder = *section_placeholders.get(&id).expect("missing placeholder");
        object
            .sections
            .builder(
                strings.get(name).map_err(|err| MergeElfError::MissingSectionName { id, err })?,
                DataSection {
                    perms: program.perms,
                    deduplication: program.deduplication,
                    bytes: program.raw,
                    relocations: relocations
                        .remove(&id)
                        .unwrap_or_default()
                        .into_iter()
                        .map(|either| match either {
                            EitherRelocation::Rel(rel) => {
                                Relocation::from_elf_rel(rel, &symbol_conversion)
                            }
                            EitherRelocation::Rela(rela) => {
                                Relocation::from_elf_rela(rela, &symbol_conversion)
                            }
                        })
                        .collect::<Result<_, _>>()?,
                    inside_relro: false,
                },
            )
            .source(source.clone())
            .retain(retain)
            .create_in_placeholder(placeholder);
    }

    object.inputs.push(Input {
        span: intern(source),
        shared_object: None,
        gnu_properties: GnuProperties { x86_isa_used, x86_features_2_used },
    });

    Ok(())
}

fn merge_symbols(
    symbols: &mut Symbols,
    section_groups: &SectionGroupsForObject<'_>,
    section_conversion: &BTreeMap<ElfSectionId, SectionId>,
    sections_not_loaded: &BTreeSet<ElfSectionId>,
    symbol_conversion: &mut BTreeMap<ElfSymbolId, SymbolId>,
    span: Interned<ObjectSpan>,
    table: ElfSymbolTable,
    strings: &Strings,
    section: &str,
) -> Result<(), MergeElfError> {
    let mut stt_file = None;
    let mut is_first = true;
    for (symbol_id, elf_symbol) in table.symbols.into_iter() {
        let resolved_name: Interned<String> =
            intern(strings.get(elf_symbol.name).map_err(|_| MergeElfError::MissingSymbolName {
                symbol: symbol_id,
                section: section.into(),
            })?);

        if is_first {
            is_first = false;

            // Instead of creating the null symbol for every object we load, we instead
            // redirect it to the shared null symbol defined during initialization.
            if resolved_name.resolve().is_empty()
                && matches!(elf_symbol.definition, ElfSymbolDefinition::Undefined)
                && matches!(elf_symbol.type_, ElfSymbolType::NoType)
            {
                symbol_conversion.insert(symbol_id, symbols.null_symbol_id());
                continue;
            }
        }

        if let ElfSymbolType::File = elf_symbol.type_ {
            stt_file = Some(resolved_name);
            continue;
        }

        // GNU AS generates symbols for each section group, pointing to the SHT_GROUP. This is not
        // really useful, as nothing can refer to that section and the SHT_GROUP wouldn't be loaded
        // in memory anyway. To avoid the linker crashing when it sees a symbol to the section that
        // wasn't loaded, we ignore all symbols pointing to a SHT_GROUP.
        if let ElfSymbolDefinition::Section(section) = &elf_symbol.definition {
            if section_groups.is_section_a_group_definition(*section) {
                continue;
            }
        }

        let id = symbols
            .add(UpcomingSymbol::Elf {
                section_conversion,
                sections_not_loaded,
                elf: elf_symbol,
                resolved_name,
                span,
                stt_file,
            })
            .map_err(|err| MergeElfError::AddSymbolFailed {
                symbol: resolved_name,
                section: section.into(),
                err,
            })?;
        symbol_conversion.insert(symbol_id, id);
    }
    Ok(())
}

enum EitherRelocation {
    Rel(ElfRel),
    Rela(ElfRela),
}

#[derive(Debug, Error, Display)]
pub(crate) enum MergeElfError {
    #[display("unsupported note with name {name} and type {type_}")]
    UnsupportedUnknownNote { name: String, type_: u32 },
    #[display("unsupported GNU property with type {type_}")]
    UnsupportedUnknownGnuProperty { type_: u32 },
    #[display("unknown section with type {id:#x?} is not supported")]
    UnsupportedUnknownSection { id: u32 },
    #[transparent]
    UnsupportedRelocation(UnsupportedRelocationType),
    #[display("unsupported dynamic symbol tables")]
    UnsupportedDynamicSymbolTable,
    #[display("loading dynamic metadata sections is not supported")]
    UnsupportedDynamicSection,
    #[display("failed to fetch section name for section {id:?}")]
    MissingSectionName {
        id: ElfSectionId,
        #[source]
        err: MissingStringError,
    },
    #[display("missing name for symbol {symbol:?} in section {section}")]
    MissingSymbolName { symbol: ElfSymbolId, section: String },
    #[display("failed to add symbol {symbol} in section {section}")]
    AddSymbolFailed {
        symbol: Interned<String>,
        section: String,
        #[source]
        err: LoadSymbolsError,
    },
    #[display("GNU property provided multiple times in the same object")]
    DuplicateGnuProperty,
    #[transparent]
    SectionGroups(SectionGroupsError),
}
