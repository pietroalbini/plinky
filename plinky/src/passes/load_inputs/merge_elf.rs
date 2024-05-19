use crate::interner::intern;
use crate::passes::load_inputs::section_groups::{SectionGroupsError, SectionGroupsForObject};
use crate::repr::object::Object;
use crate::repr::relocations::UnsupportedRelocationType;
use crate::repr::sections::{DataSection, Section, SectionContent, UninitializedSection};
use crate::repr::strings::MissingStringError;
use crate::repr::symbols::LoadSymbolsError;
use plinky_diagnostics::ObjectSpan;
use plinky_elf::ids::serial::{SectionId, SerialIds};
use plinky_elf::{ElfNote, ElfObject, ElfSectionContent};
use plinky_macros::{Display, Error};
use std::collections::BTreeMap;

pub(super) fn merge(
    ids: &mut SerialIds,
    object: &mut Object,
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
            ElfSectionContent::StringTable(table) => object.strings.load_table(section_id, table),
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
        section_groups.add_group(&object.strings, &symbol_tables, id, group)?;
    }

    // This is loaded after the string tables are loaded by the previous iteration, as we need
    // to resolve the strings as part of symbol loading.
    for (name_id, mut table) in symbol_tables {
        section_groups.filter_symbol_table(&mut table)?;
        object.symbols.load_table(ids, intern(source.clone()), table, &object.strings).map_err(
            |inner| MergeElfError::SymbolsLoadingFailed {
                section_name: object.strings.get(name_id).unwrap_or("<unknown>").into(),
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
                object
                    .strings
                    .get(name)
                    .map_err(|err| MergeElfError::MissingSectionName { id, err })?,
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
                object
                    .strings
                    .get(name)
                    .map_err(|err| MergeElfError::MissingSectionName { id, err })?,
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
