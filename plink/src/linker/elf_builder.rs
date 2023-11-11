use crate::linker::layout::{SectionLayout, SectionMerge};
use crate::linker::object::{GetSymbolAddressError, Object};
use plink_elf::ids::serial::{SectionId, SerialIds, StringId};
use plink_elf::{
    ElfEnvironment, ElfObject, ElfProgramSection, ElfSection, ElfSectionContent, ElfSegment,
    ElfSegmentContent, ElfSegmentType, ElfStringTable, ElfType, RawBytes,
};
use plink_macros::Error;
use std::collections::BTreeMap;
use std::num::NonZeroU64;

pub(super) struct ElfBuilderContext {
    pub(super) entrypoint: String,
    pub(super) env: ElfEnvironment,
    pub(super) object: Object<SectionLayout>,
    pub(super) section_merges: Vec<SectionMerge>,
}

pub(super) struct ElfBuilder {
    ctx: ElfBuilderContext,
    ids: SerialIds,
    section_names: PendingStringsTable,
    section_zero_id: SectionId,
}

impl ElfBuilder {
    pub(super) fn new(ctx: ElfBuilderContext) -> Self {
        let mut ids = SerialIds::new();
        Self {
            ctx,
            section_zero_id: ids.allocate_section_id(),
            section_names: PendingStringsTable::new(&mut ids),
            ids,
        }
    }

    pub(super) fn build(mut self) -> Result<ElfObject<SerialIds>, ElfBuilderError> {
        let entry = self.prepare_entry_point()?;
        let sections = self.prepare_sections();
        let segments = self.prepare_segments(&sections);

        Ok(ElfObject {
            env: self.ctx.env,
            type_: ElfType::Executable,
            entry,
            flags: 0,
            sections,
            segments,
        })
    }

    fn prepare_entry_point(&self) -> Result<Option<NonZeroU64>, ElfBuilderError> {
        Ok(Some(
            NonZeroU64::new(
                self.ctx
                    .object
                    .global_symbol_address(&self.ctx.entrypoint)
                    .map_err(ElfBuilderError::InvalidEntrypoint)?,
            )
            .ok_or_else(|| ElfBuilderError::EntrypointIsZero(self.ctx.entrypoint.clone()))?,
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

        while let Some(merge) = self.ctx.section_merges.pop() {
            sections.insert(
                self.ids.allocate_section_id(),
                self.prepare_program_section(merge),
            );
        }

        sections.insert(self.section_names.id, self.prepare_section_names_table());

        sections
    }

    fn prepare_program_section(&mut self, merge: SectionMerge) -> ElfSection<SerialIds> {
        let mut bytes = Vec::new();
        for section in merge.sections {
            let section = self.ctx.object.take_program_section(section);
            bytes.extend_from_slice(&section.raw.0);
        }

        ElfSection {
            name: self.section_names.add(&merge.name),
            memory_address: merge.address,
            content: ElfSectionContent::Program(ElfProgramSection {
                perms: merge.perms,
                raw: RawBytes(bytes),
            }),
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
            if let ElfSectionContent::Program(program) = &section.content {
                segments.push(ElfSegment {
                    type_: ElfSegmentType::Load,
                    perms: program.perms,
                    content: vec![ElfSegmentContent::Section(*section_id)],
                    align: 0x1000,
                });
            }
        }
        segments
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
