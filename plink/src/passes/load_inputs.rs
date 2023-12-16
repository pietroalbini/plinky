use crate::interner::{intern, Interned};
use crate::repr::object::{
    DataSection, DataSectionPart, Object, Section, SectionContent, UninitializedSectionPart,
};
use crate::repr::strings::{MissingStringError, Strings};
use crate::repr::symbols::{LoadSymbolsError, Symbols};
use plink_elf::errors::LoadError;
use plink_elf::ids::serial::{SectionId, SerialIds, StringId};
use plink_elf::{
    ElfDeduplication, ElfEnvironment, ElfNote, ElfObject, ElfPermissions, ElfSectionContent,
};
use plink_macros::{Display, Error};
use std::collections::BTreeMap;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;

pub(crate) fn run(paths: &[PathBuf], ids: &mut SerialIds) -> Result<Object<()>, LoadInputsError> {
    let mut state = None;

    for path in paths {
        let mut file = File::open(path).map_err(|e| LoadInputsError::OpenFailed(path.into(), e))?;
        let elf = ElfObject::load(&mut BufReader::new(&mut file), ids)
            .map_err(|e| LoadInputsError::ParseError(path.into(), e))?;

        match &mut state {
            None => {
                let mut object = Object {
                    env: elf.env,
                    sections: BTreeMap::new(),
                    section_ids_to_names: BTreeMap::new(),
                    strings: Strings::new(),
                    symbols: Symbols::new(),
                };
                merge_elf(&mut object, elf)?;
                state = Some((object, path));
            }
            Some((object, first_path)) => {
                if object.env != elf.env {
                    return Err(LoadInputsError::MismatchedEnv {
                        first_path: (*first_path).into(),
                        first_env: object.env,
                        current_path: path.into(),
                        current_env: elf.env,
                    });
                }
                merge_elf(object, elf)?;
            }
        }
    }

    state.map(|(o, _)| o).ok_or(LoadInputsError::NoInputFiles)
}

fn merge_elf(object: &mut Object<()>, elf: ElfObject<SerialIds>) -> Result<(), LoadInputsError> {
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
                            return Err(LoadInputsError::UnsupportedUnknownNote {
                                name: unknown.name,
                                type_: unknown.type_,
                            })
                        }
                    }
                }
            }
            ElfSectionContent::Unknown(unknown) => {
                return Err(LoadInputsError::UnsupportedUnknownSection { id: unknown.id });
            }
        }
    }

    // This is loaded after the string tables are loaded by the previous iteration, as we need
    // to resolve the strings as part of symbol loading.
    for (name_id, table) in symbol_tables {
        object
            .symbols
            .load_table(table, &object.strings)
            .map_err(|inner| LoadInputsError::SymbolsLoadingFailed {
                section_name: object.strings.get(name_id).unwrap_or("<unknown>").into(),
                inner,
            })?;
    }

    for (id, name, uninit) in uninitialized_sections {
        add_section(
            object,
            id,
            name,
            uninit.perms,
            || SectionContent::Uninitialized(BTreeMap::new()),
            |name, content| match content {
                SectionContent::Data(_) => Err(LoadInputsError::MismatchedSectionTypes {
                    name,
                    first_type: "data",
                    second_type: "uninitialized",
                }),
                SectionContent::Uninitialized(c) => {
                    c.insert(
                        id,
                        UninitializedSectionPart {
                            len: uninit.len,
                            layout: (),
                        },
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
                SectionContent::Uninitialized(_) => Err(LoadInputsError::MismatchedSectionTypes {
                    name,
                    first_type: "uninitialized",
                    second_type: "data",
                }),
                SectionContent::Data(c) => {
                    if c.deduplication != program.deduplication {
                        return Err(LoadInputsError::MismatchedDeduplication {
                            name,
                            first_dedup: c.deduplication,
                            second_dedup: program.deduplication,
                        });
                    }
                    c.parts.insert(
                        id,
                        DataSectionPart {
                            bytes: program.raw,
                            relocations: relocations.remove(&id).unwrap_or_else(Vec::new),
                            layout: (),
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
    object: &mut Object<()>,
    id: SectionId,
    name: StringId,
    perms: ElfPermissions,
    init_content: I,
    update_content: U,
) -> Result<(), LoadInputsError>
where
    I: FnOnce() -> SectionContent<()>,
    U: FnOnce(Interned<String>, &mut SectionContent<()>) -> Result<(), LoadInputsError>,
{
    let name = intern(
        object
            .strings
            .get(name)
            .map_err(|err| LoadInputsError::MissingSectionName { id, err })?,
    );

    let section = object.sections.entry(name).or_insert_with(|| Section {
        perms,
        content: init_content(),
    });
    if section.perms != perms {
        return Err(LoadInputsError::MismatchedSectionPerms {
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
pub(crate) enum LoadInputsError {
    #[display("failed to open file {f0:?}")]
    OpenFailed(PathBuf, #[source] std::io::Error),
    #[display("failed to parse ELF file at {f0:?}")]
    ParseError(PathBuf, #[source] LoadError),
    #[display("no input files were provided")]
    NoInputFiles,
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
    #[display("environment of {first_path:?} is {first_env:?}, while environment of {current_path:?} is {current_env:?}")]
    MismatchedEnv {
        first_path: PathBuf,
        first_env: ElfEnvironment,
        current_path: PathBuf,
        current_env: ElfEnvironment,
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
