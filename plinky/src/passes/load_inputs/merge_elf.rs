use crate::interner::{intern, Interned};
use crate::passes::load_inputs::section_groups::{SectionGroupsError, SectionGroupsForObject};
use crate::passes::load_inputs::strings::{MissingStringError, Strings};
use crate::repr::object::Object;
use crate::repr::relocations::{Relocation, UnsupportedRelocationType};
use crate::repr::sections::{DataSection, UninitializedSection};
use crate::repr::symbols::{LoadSymbolsError, Symbol, Symbols};
use crate::utils::before_freeze::BeforeFreeze;
use plinky_diagnostics::ObjectSpan;
use plinky_elf::ids::serial::{SectionId, SerialIds};
use plinky_elf::{
    ElfNote, ElfObject, ElfSectionContent, ElfSymbolDefinition, ElfSymbolTable, ElfSymbolType,
};
use plinky_macros::{Display, Error};
use std::collections::BTreeMap;

pub(super) fn merge(
    object: &mut Object,
    strings: &mut Strings,
    mut section_groups: SectionGroupsForObject<'_>,
    source: ObjectSpan,
    elf: ElfObject<SerialIds>,
    before_freeze: &BeforeFreeze,
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
            ElfSectionContent::SymbolTable(table) => {
                if table.dynsym {
                    return Err(MergeElfError::UnsupportedDynamicSymbolTable);
                }
                symbol_tables.push((section.name, table))
            }
            ElfSectionContent::StringTable(table) => strings.load_table(section_id, table),
            ElfSectionContent::RelocationsTable(table) => {
                relocations.insert(table.applies_to_section, table.relocations);
            }
            ElfSectionContent::Group(group) => pending_groups.push((section_id, group)),
            ElfSectionContent::Hash(_) => {
                // We don't need hash tables imported from the ELF file, we build our own.
            }
            ElfSectionContent::Dynamic(_) => {
                return Err(MergeElfError::UnsupportedDynamicSection);
            }
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
        merge_symbols(&mut object.symbols, intern(source.clone()), table, &strings, before_freeze)
            .map_err(|inner| MergeElfError::SymbolsLoadingFailed {
                section_name: strings.get(name_id).unwrap_or("<unknown>").into(),
                inner,
            })?;
    }

    for (id, name, uninit) in uninitialized_sections {
        if section_groups.should_skip_section(id) {
            continue;
        }
        object
            .sections
            .builder(
                strings.get(name).map_err(|err| MergeElfError::MissingSectionName { id, err })?,
                UninitializedSection { perms: uninit.perms, len: uninit.len },
            )
            .source(source.clone())
            .create_with_id(id);
    }

    for (id, name, program) in program_sections {
        if section_groups.should_skip_section(id) {
            continue;
        }
        object
            .sections
            .builder(
                strings.get(name).map_err(|err| MergeElfError::MissingSectionName { id, err })?,
                DataSection {
                    perms: program.perms,
                    deduplication: program.deduplication,
                    bytes: program.raw.0,
                    relocations: relocations
                        .remove(&id)
                        .unwrap_or_default()
                        .into_iter()
                        .map(|r| Relocation::from_elf(id, r))
                        .collect::<Result<_, _>>()?,
                },
            )
            .source(source.clone())
            .create_with_id(id);
    }
    Ok(())
}

fn merge_symbols(
    symbols: &mut Symbols,
    span: Interned<ObjectSpan>,
    table: ElfSymbolTable<SerialIds>,
    strings: &Strings,
    before_freeze: &BeforeFreeze,
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
                symbols.add_redirect(symbol_id, symbols.null_symbol_id(), before_freeze);
                continue;
            }
        }

        if let ElfSymbolType::File = elf_symbol.type_ {
            stt_file = Some(name);
            continue;
        }

        symbols.add_symbol(
            Symbol::new_elf(symbol_id, elf_symbol, name, span, stt_file)?,
            before_freeze,
        )?;
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
    #[display("unsupported dynamic symbol tables")]
    UnsupportedDynamicSymbolTable,
    #[display("loading dynamic metadata sections is not supported")]
    UnsupportedDynamicSection,
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
