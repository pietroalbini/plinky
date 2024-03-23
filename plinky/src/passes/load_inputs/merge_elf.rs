use crate::interner::{intern, Interned};
use crate::repr::object::{
    DataSection, DataSectionPart, Object, Section, SectionContent, UninitializedSectionPart,
};
use crate::repr::strings::MissingStringError;
use crate::repr::symbols::LoadSymbolsError;
use plinky_diagnostics::ObjectSpan;
use plinky_elf::ids::serial::{SectionId, SerialIds, StringId};
use plinky_elf::{ElfDeduplication, ElfNote, ElfObject, ElfPermissions, ElfSectionContent};
use plinky_macros::{Display, Error};
use std::collections::BTreeMap;

pub(super) fn merge(
    ids: &mut SerialIds,
    object: &mut Object,
    source: ObjectSpan,
    elf: ElfObject<SerialIds>,
) -> Result<(), MergeElfError> {
    let mut symbol_tables = Vec::new();
    let mut program_sections = Vec::new();
    let mut uninitialized_sections = Vec::new();
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

    // This is loaded after the string tables are loaded by the previous iteration, as we need
    // to resolve the strings as part of symbol loading.
    for (name_id, table) in symbol_tables {
        object.symbols.load_table(ids, intern(source.clone()), table, &object.strings).map_err(
            |inner| MergeElfError::SymbolsLoadingFailed {
                section_name: object.strings.get(name_id).unwrap_or("<unknown>").into(),
                inner,
            },
        )?;
    }

    for (id, name, uninit) in uninitialized_sections {
        add_section(
            object,
            id,
            name,
            uninit.perms,
            || SectionContent::Uninitialized(BTreeMap::new()),
            |name, content| match content {
                SectionContent::Data(_) => Err(MergeElfError::MismatchedSectionTypes {
                    name,
                    first_type: "data",
                    second_type: "uninitialized",
                }),
                SectionContent::Uninitialized(c) => {
                    c.insert(
                        id,
                        UninitializedSectionPart { source: source.clone(), len: uninit.len },
                    );
                    Ok(())
                }
            },
        )?;
    }

    for (id, name, program) in program_sections {
        add_section(
            object,
            id,
            name,
            program.perms,
            || {
                SectionContent::Data(DataSection {
                    deduplication: program.deduplication,
                    parts: BTreeMap::new(),
                })
            },
            |name, content| match content {
                SectionContent::Uninitialized(_) => Err(MergeElfError::MismatchedSectionTypes {
                    name,
                    first_type: "uninitialized",
                    second_type: "data",
                }),
                SectionContent::Data(c) => {
                    if c.deduplication != program.deduplication {
                        return Err(MergeElfError::MismatchedDeduplication {
                            name,
                            first_dedup: c.deduplication,
                            second_dedup: program.deduplication,
                        });
                    }
                    c.parts.insert(
                        id,
                        DataSectionPart {
                            source: source.clone(),
                            bytes: program.raw,
                            relocations: relocations.remove(&id).unwrap_or_else(Vec::new),
                        },
                    );
                    Ok(())
                }
            },
        )?;
    }
    Ok(())
}

fn add_section<I, U>(
    object: &mut Object,
    id: SectionId,
    name: StringId,
    perms: ElfPermissions,
    init_content: I,
    update_content: U,
) -> Result<(), MergeElfError>
where
    I: FnOnce() -> SectionContent,
    U: FnOnce(Interned<String>, &mut SectionContent) -> Result<(), MergeElfError>,
{
    let name = intern(
        object.strings.get(name).map_err(|err| MergeElfError::MissingSectionName { id, err })?,
    );

    let section =
        object.sections.entry(name).or_insert_with(|| Section { perms, content: init_content() });
    if section.perms != perms {
        return Err(MergeElfError::MismatchedSectionPerms {
            name,
            first_perms: section.perms,
            second_perms: perms,
        });
    }
    update_content(name, &mut section.content)?;

    object.section_ids_to_names.insert(id, name);
    Ok(())
}

#[derive(Debug, Error, Display)]
pub(crate) enum MergeElfError {
    #[display("unsupported note with name {name} and type {type_}")]
    UnsupportedUnknownNote { name: String, type_: u32 },
    #[display("unknown section with type {id:#x?} is not supported")]
    UnsupportedUnknownSection { id: u32 },
    #[display("failed to load symbols from section {section_name}")]
    SymbolsLoadingFailed {
        section_name: String,
        #[source]
        inner: LoadSymbolsError,
    },
    #[display("instances of section {name} have different types: one is of type {first_type}, while the other is of type {second_type}")]
    MismatchedSectionTypes {
        name: Interned<String>,
        first_type: &'static str,
        second_type: &'static str,
    },
    #[display("instances of section {name} have different permissions: one is {first_perms:?}, while the other is {second_perms:?}")]
    MismatchedSectionPerms {
        name: Interned<String>,
        first_perms: ElfPermissions,
        second_perms: ElfPermissions,
    },
    #[display("instances of section {name} have different deduplications: one is {first_dedup:?}, while the other is {second_dedup:?}")]
    MismatchedDeduplication {
        name: Interned<String>,
        first_dedup: ElfDeduplication,
        second_dedup: ElfDeduplication,
    },
    #[display("failed to fetch section name for section {id:?}")]
    MissingSectionName {
        id: SectionId,
        #[source]
        err: MissingStringError,
    },
}
