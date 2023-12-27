use crate::cli::CliOptions;
use crate::interner::Interned;
use crate::repr::object::{
    DataSectionPart, GetSymbolAddressError, Object, Section, SectionContent, SectionLayout,
};
use plinky_elf::ids::serial::{SectionId, SerialIds, StringId};
use plinky_elf::{
    ElfObject, ElfProgramSection, ElfSection, ElfSectionContent, ElfSegment, ElfSegmentContent,
    ElfSegmentType, ElfStringTable, ElfType, ElfUninitializedSection, RawBytes,
};
use plinky_macros::Error;
use std::collections::BTreeMap;
use std::num::NonZeroU64;

pub(crate) fn run(
    object: Object<SectionLayout>,
    options: &CliOptions,
) -> Result<ElfObject<SerialIds>, ElfBuilderError> {
    let mut ids = SerialIds::new();
    let builder = ElfBuilder {
        entrypoint: options.entry.clone(),
        object,
        section_zero_id: ids.allocate_section_id(),
        section_names: PendingStringsTable::new(&mut ids),
        ids,
    };
    builder.build()
}

struct ElfBuilder {
    entrypoint: String,
    object: Object<SectionLayout>,

    ids: SerialIds,
    section_names: PendingStringsTable,
    section_zero_id: SectionId,
}

impl ElfBuilder {
    fn build(mut self) -> Result<ElfObject<SerialIds>, ElfBuilderError> {
        let entry = self.prepare_entry_point()?;
        let sections = self.prepare_sections();
        let segments = self.prepare_segments(&sections);

        Ok(ElfObject {
            env: self.object.env,
            type_: ElfType::Executable,
            entry,
            sections,
            segments,
        })
    }

    fn prepare_entry_point(&self) -> Result<Option<NonZeroU64>, ElfBuilderError> {
        Ok(Some(
            NonZeroU64::new(
                self.object
                    .global_symbol_address(&self.entrypoint)
                    .map_err(ElfBuilderError::InvalidEntrypoint)?,
            )
            .ok_or_else(|| ElfBuilderError::EntrypointIsZero(self.entrypoint.clone()))?,
        ))
    }

    fn prepare_sections(&mut self) -> BTreeMap<SectionId, ElfSection<SerialIds>> {
        let mut sections = BTreeMap::new();

        // The first section must always be the null section.
        sections.insert(
            self.section_zero_id,
            ElfSection {
                name: StringId::new(self.section_names.id, 0),
                memory_address: 0,
                content: ElfSectionContent::Null,
            },
        );

        while let Some((name, section)) = self.object.sections.pop_first() {
            sections.insert(self.ids.allocate_section_id(), self.prepare_section(name, section));
        }

        sections.insert(self.section_names.id, self.prepare_section_names_table());

        sections
    }

    fn prepare_section(
        &mut self,
        name: Interned<String>,
        section: Section<SectionLayout>,
    ) -> ElfSection<SerialIds> {
        let mut memory_address: Option<u64> = None;
        let mut update_memory_address = |new| match memory_address {
            None => memory_address = Some(new),
            Some(existing) => memory_address = Some(existing.min(new)),
        };

        let content = match section.content {
            SectionContent::Data(data) => {
                let mut raw = Vec::new();
                for part in data.parts.into_values() {
                    match part {
                        DataSectionPart::Real(real) => {
                            update_memory_address(real.layout.address);
                            raw.extend_from_slice(&real.bytes);
                        }
                        // We shouldn't write deduplication facades.
                        DataSectionPart::DeduplicationFacade(_) => {}
                    }
                }
                ElfSectionContent::Program(ElfProgramSection {
                    perms: section.perms,
                    deduplication: data.deduplication,
                    raw: RawBytes(raw),
                })
            }
            SectionContent::Uninitialized(uninit) => {
                let mut len = 0;
                for part in uninit.into_values() {
                    update_memory_address(part.layout.address);
                    len += part.len;
                }
                ElfSectionContent::Uninitialized(ElfUninitializedSection {
                    perms: section.perms,
                    len,
                })
            }
        };

        ElfSection {
            name: self.section_names.add(&name.resolve()),
            memory_address: memory_address.expect("empty section"),
            content,
        }
    }

    fn prepare_section_names_table(&mut self) -> ElfSection<SerialIds> {
        let name = self.section_names.add(".shstrtab");
        ElfSection {
            name,
            memory_address: 0,
            content: ElfSectionContent::StringTable(ElfStringTable::new(
                self.section_names.strings.clone(),
            )),
        }
    }

    fn prepare_segments(
        &self,
        sections: &BTreeMap<SectionId, ElfSection<SerialIds>>,
    ) -> Vec<ElfSegment<SerialIds>> {
        let mut segments = Vec::new();
        for (section_id, section) in sections.iter() {
            match &section.content {
                ElfSectionContent::Program(program) => {
                    segments.push((
                        section.memory_address,
                        ElfSegment {
                            type_: ElfSegmentType::Load,
                            perms: program.perms,
                            content: vec![ElfSegmentContent::Section(*section_id)],
                            align: 0x1000,
                        },
                    ));
                }
                ElfSectionContent::Uninitialized(uninit) => {
                    segments.push((
                        section.memory_address,
                        ElfSegment {
                            type_: ElfSegmentType::Load,
                            perms: uninit.perms,
                            content: vec![ElfSegmentContent::Section(*section_id)],
                            align: 0x1000,
                        },
                    ));
                }
                _ => (),
            }
        }

        // Segments have to be in order in memory, otherwise they will not be loaded.
        segments.sort_by_key(|(addr, _segment)| *addr);
        segments.into_iter().map(|(_addr, segment)| segment).collect()
    }
}

struct PendingStringsTable {
    id: SectionId,
    strings: BTreeMap<u32, String>,
    next_offset: u32,
}

impl PendingStringsTable {
    fn new(ids: &mut SerialIds) -> Self {
        let mut strings = BTreeMap::new();
        strings.insert(0, String::new()); // First string has to always be empty.
        Self { id: ids.allocate_section_id(), strings, next_offset: 1 }
    }

    fn add(&mut self, string: &str) -> StringId {
        let offset = self.next_offset;
        self.next_offset += string.len() as u32 + 1;
        self.strings.insert(offset, string.into());
        StringId::new(self.id, offset)
    }
}

#[derive(Debug, Error)]
pub(crate) enum ElfBuilderError {
    InvalidEntrypoint(#[source] GetSymbolAddressError),
    EntrypointIsZero(String),
}

impl std::fmt::Display for ElfBuilderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ElfBuilderError::InvalidEntrypoint(_) => {
                f.write_str("failed to find the entry point of the executable")
            }
            ElfBuilderError::EntrypointIsZero(entrypoint) => {
                write!(f, "entry point symbol {entrypoint} is zero")
            }
        }
    }
}
