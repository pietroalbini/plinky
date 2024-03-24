use super::layout::SectionLayout;
use crate::cli::CliOptions;
use crate::interner::{intern, Interned};
use crate::passes::layout::Layout;
use crate::repr::object::{Object, Section, SectionContent};
use crate::repr::symbols::{ResolveSymbolError, ResolveSymbolErrorKind, ResolvedSymbol};
use plinky_elf::ids::serial::{SectionId, SerialIds, StringId};
use plinky_elf::{
    ElfObject, ElfProgramSection, ElfSection, ElfSectionContent, ElfSegment, ElfSegmentContent,
    ElfSegmentType, ElfStringTable, ElfType, ElfUninitializedSection, RawBytes,
};
use plinky_macros::{Display, Error};
use std::collections::BTreeMap;
use std::num::NonZeroU64;

pub(crate) fn run(
    object: Object,
    layout: &Layout,
    options: &CliOptions,
) -> Result<ElfObject<SerialIds>, ElfBuilderError> {
    let mut ids = SerialIds::new();
    let builder = ElfBuilder {
        entrypoint: intern(&options.entry),
        object,
        layout,
        section_zero_id: ids.allocate_section_id(),
        section_names: PendingStringsTable::new(&mut ids),
        ids,
    };
    builder.build()
}

struct ElfBuilder<'a> {
    entrypoint: Interned<String>,
    object: Object,
    layout: &'a Layout,

    ids: SerialIds,
    section_names: PendingStringsTable,
    section_zero_id: SectionId,
}

impl ElfBuilder<'_> {
    fn build(mut self) -> Result<ElfObject<SerialIds>, ElfBuilderError> {
        let entry = self.prepare_entry_point()?;
        let (sections, sections_segment) = self.prepare_sections();
        let segments = self.prepare_segments(&sections, &sections_segment);

        Ok(ElfObject {
            env: self.object.env,
            type_: ElfType::Executable,
            entry,
            sections,
            segments,
        })
    }

    fn prepare_entry_point(&self) -> Result<Option<NonZeroU64>, ElfBuilderError> {
        let resolved = self
            .object
            .symbols
            .get_global(self.entrypoint)
            .map_err(|_| {
                ElfBuilderError::EntryPointResolution(ResolveSymbolError {
                    symbol: self.entrypoint,
                    inner: ResolveSymbolErrorKind::Undefined,
                })
            })?
            .resolve(self.layout, 0)
            .map_err(ElfBuilderError::EntryPointResolution)?;

        match resolved {
            ResolvedSymbol::Absolute(_) => {
                Err(ElfBuilderError::EntryPointNotAnAddress(self.entrypoint))
            }
            ResolvedSymbol::Address(addr) => Ok(Some(
                NonZeroU64::new(addr).ok_or(ElfBuilderError::EntrypointIsZero(self.entrypoint))?,
            )),
        }
    }

    fn prepare_sections(
        &mut self,
    ) -> (BTreeMap<SectionId, ElfSection<SerialIds>>, BTreeMap<SectionId, SectionSegment>) {
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

        let mut sections_segment = BTreeMap::new();
        while let Some((name, section)) = self.object.sections.pop_first() {
            let (elf_section, section_segment) = self.prepare_section(name, section);
            let id = self.ids.allocate_section_id();
            sections.insert(id, elf_section);
            sections_segment.insert(id, section_segment);
        }

        sections.insert(self.section_names.id, self.prepare_section_names_table());

        (sections, sections_segment)
    }

    fn prepare_section(
        &mut self,
        name: Interned<String>,
        section: Section,
    ) -> (ElfSection<SerialIds>, SectionSegment) {
        let mut memory_address: Option<u64> = None;
        let mut section_segment = SectionSegment::Exclude;
        let mut update_memory_address = |new: &SectionLayout| {
            let new = match new {
                SectionLayout::Allocated { address } => {
                    section_segment = SectionSegment::Include;
                    *address
                }
                SectionLayout::NotAllocated => 0,
            };
            match memory_address {
                None => memory_address = Some(new),
                Some(existing) => memory_address = Some(existing.min(new)),
            }
        };

        let content = match section.content {
            SectionContent::Data(data) => {
                let mut raw = Vec::new();
                for (id, part) in data.parts.into_iter() {
                    update_memory_address(self.layout.of_section(id));
                    raw.extend_from_slice(&part.bytes);
                }
                ElfSectionContent::Program(ElfProgramSection {
                    perms: section.perms,
                    deduplication: data.deduplication,
                    raw: RawBytes(raw),
                })
            }
            SectionContent::Uninitialized(uninit) => {
                let mut len = 0;
                for (id, part) in uninit.into_iter() {
                    update_memory_address(self.layout.of_section(id));
                    len += part.len;
                }
                ElfSectionContent::Uninitialized(ElfUninitializedSection {
                    perms: section.perms,
                    len,
                })
            }
        };

        (
            ElfSection {
                name: self.section_names.add(&name.resolve()),
                memory_address: memory_address.expect("empty section"),
                content,
            },
            section_segment,
        )
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
        sections_segment: &BTreeMap<SectionId, SectionSegment>,
    ) -> Vec<ElfSegment<SerialIds>> {
        let mut segments = Vec::new();
        for (section_id, section) in sections.iter() {
            if sections_segment.get(section_id) != Some(&SectionSegment::Include) {
                continue;
            }
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

#[derive(PartialEq, Eq)]
enum SectionSegment {
    Include,
    Exclude,
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

#[derive(Debug, Error, Display)]
pub(crate) enum ElfBuilderError {
    #[display("failed to resolve the entry point")]
    EntryPointResolution(#[source] ResolveSymbolError),
    #[display("entry point symbol {f0} is not an address")]
    EntryPointNotAnAddress(Interned<String>),
    #[display("the entry point is zero")]
    EntrypointIsZero(Interned<String>),
}
