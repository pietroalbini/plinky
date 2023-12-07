use crate::cli::CliOptions;
use crate::repr::object::{GetSymbolAddressError, Object, SectionLayout, SectionMerge, SectionContent};
use plink_elf::ids::serial::{SectionId, SerialIds, StringId};
use plink_elf::{
    ElfDeduplication, ElfObject, ElfProgramSection, ElfSection, ElfSectionContent, ElfSegment,
    ElfSegmentContent, ElfSegmentType, ElfStringTable, ElfType, ElfUninitializedSection,
};
use plink_macros::Error;
use std::collections::BTreeMap;
use std::num::NonZeroU64;

pub(crate) fn run(
    object: Object<SectionLayout>,
    section_merges: Vec<SectionMerge>,
    options: &CliOptions,
) -> Result<ElfObject<SerialIds>, ElfBuilderError> {
    let mut ids = SerialIds::new();
    let builder = ElfBuilder {
        entrypoint: options.entry.clone(),
        object,
        section_merges,
        section_zero_id: ids.allocate_section_id(),
        section_names: PendingStringsTable::new(&mut ids),
        ids,
    };
    builder.build()
}

struct ElfBuilder {
    entrypoint: String,
    object: Object<SectionLayout>,
    section_merges: Vec<SectionMerge>,

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

        while let Some(merge) = self.section_merges.pop() {
            if let Some(section) = self.prepare_section(merge) {
                sections.insert(self.ids.allocate_section_id(), section);
            }
        }

        sections.insert(self.section_names.id, self.prepare_section_names_table());

        sections
    }

    fn prepare_section(&mut self, merge: SectionMerge) -> Option<ElfSection<SerialIds>> {
        let mut content = None;
        for section in merge.sections {
            let section = self.object.take_section(section);
            match (&mut content, section.content) {
                (None, SectionContent::Data(data)) => {
                    content = Some(ElfSectionContent::Program(ElfProgramSection {
                        perms: merge.perms,
                        deduplication: ElfDeduplication::Disabled, // TODO: implement merging.
                        raw: data.bytes,
                    }));
                }
                (None, SectionContent::Uninitialized(uninit)) => {
                    content = Some(ElfSectionContent::Uninitialized(ElfUninitializedSection {
                        perms: merge.perms,
                        len: uninit.len,
                    }));
                }
                (Some(ElfSectionContent::Program(elf)), SectionContent::Data(data)) => {
                    elf.raw.0.extend_from_slice(&data.bytes.0);
                }
                (Some(ElfSectionContent::Uninitialized(elf)), SectionContent::Uninitialized(u)) => {
                    elf.len += u.len;
                }
                _ => panic!("mixed different section content types in the same merge"),
            }
        }

        Some(ElfSection {
            name: self.section_names.add(&merge.name),
            memory_address: merge.address,
            content: content?,
        })
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
        segments
            .into_iter()
            .map(|(_addr, segment)| segment)
            .collect()
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
        Self {
            id: ids.allocate_section_id(),
            strings,
            next_offset: 1,
        }
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
