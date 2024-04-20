pub(crate) mod ids;

use super::layout::SectionLayout;
use crate::interner::Interned;
use crate::passes::build_elf::ids::{BuiltElfIds, BuiltElfSectionId, BuiltElfStringId};
use crate::passes::layout::Layout;
use crate::repr::object::Object;
use crate::repr::sections::SectionContent;
use crate::repr::symbols::{ResolveSymbolError, ResolvedSymbol};
use plinky_elf::ids::serial::SectionId;
use plinky_elf::{
    ElfObject, ElfPermissions, ElfProgramSection, ElfSection, ElfSectionContent, ElfSegment,
    ElfSegmentContent, ElfSegmentType, ElfStringTable, ElfType, ElfUninitializedSection, RawBytes,
};
use plinky_macros::{Display, Error};
use std::collections::BTreeMap;
use std::num::NonZeroU64;

pub(crate) fn run(
    object: Object,
    layout: &Layout,
) -> Result<ElfObject<BuiltElfIds>, ElfBuilderError> {
    let mut ids = BuiltElfIds::new();
    let builder = ElfBuilder {
        object,
        layout,
        section_zero_id: ids.allocate_section_id(),
        section_names: PendingStringsTable::new(&mut ids),
        section_ids_mapping: BTreeMap::new(),
        ids,
    };
    builder.build()
}

struct ElfBuilder<'a> {
    object: Object,
    layout: &'a Layout,

    section_ids_mapping: BTreeMap<SectionId, BuiltElfSectionId>,

    ids: BuiltElfIds,
    section_names: PendingStringsTable,
    section_zero_id: BuiltElfSectionId,
}

impl ElfBuilder<'_> {
    fn build(mut self) -> Result<ElfObject<BuiltElfIds>, ElfBuilderError> {
        let entry = self.prepare_entry_point()?;
        let sections = self.prepare_sections();
        let segments = self.prepare_segments();

        Ok(ElfObject {
            env: self.object.env,
            type_: ElfType::Executable,
            entry,
            sections,
            segments,
        })
    }

    fn prepare_entry_point(&self) -> Result<Option<NonZeroU64>, ElfBuilderError> {
        let symbol = self.object.symbols.get(self.object.entry_point);
        let resolved =
            symbol.resolve(self.layout, 0).map_err(ElfBuilderError::EntryPointResolution)?;

        match resolved {
            ResolvedSymbol::Absolute(_) => {
                Err(ElfBuilderError::EntryPointNotAnAddress(symbol.name))
            }
            ResolvedSymbol::Address(addr) => Ok(Some(
                NonZeroU64::new(addr).ok_or(ElfBuilderError::EntrypointIsZero(symbol.name))?,
            )),
        }
    }

    fn prepare_sections(&mut self) -> BTreeMap<BuiltElfSectionId, ElfSection<BuiltElfIds>> {
        let mut sections = BTreeMap::new();

        // The first section must always be the null section.
        sections.insert(
            self.section_zero_id,
            ElfSection {
                name: BuiltElfStringId::new(self.section_names.id, 0),
                memory_address: 0,
                content: ElfSectionContent::Null,
            },
        );

        while let Some(section) = self.object.sections.pop_first() {
            match &section.content {
                SectionContent::Data(data) => {
                    let new_id = self.ids.allocate_section_id();
                    let layout = self.layout.of_section(section.id);
                    sections.insert(
                        new_id,
                        ElfSection {
                            name: self.section_names.add(&section.name.resolve()),
                            memory_address: match layout {
                                SectionLayout::Allocated { address } => *address,
                                SectionLayout::NotAllocated => 0,
                            },
                            content: ElfSectionContent::Program(ElfProgramSection {
                                perms: section.perms,
                                deduplication: data.deduplication,
                                raw: RawBytes(data.bytes.clone()),
                            }),
                        },
                    );
                    self.section_ids_mapping.insert(section.id, new_id);
                }
                SectionContent::Uninitialized(uninit) => {
                    let new_id = self.ids.allocate_section_id();
                    let layout = self.layout.of_section(section.id);
                    sections.insert(
                        new_id,
                        ElfSection {
                            name: self.section_names.add(&section.name.resolve()),
                            memory_address: match layout {
                                SectionLayout::Allocated { address } => *address,
                                SectionLayout::NotAllocated => 0,
                            },
                            content: ElfSectionContent::Uninitialized(ElfUninitializedSection {
                                perms: section.perms,
                                len: uninit.len,
                            }),
                        },
                    );
                    self.section_ids_mapping.insert(section.id, new_id);
                }
            }
        }

        sections.insert(self.section_names.id, self.prepare_section_names_table());
        sections
    }

    fn prepare_section_names_table(&mut self) -> ElfSection<BuiltElfIds> {
        let name = self.section_names.add(".shstrtab");
        ElfSection {
            name,
            memory_address: 0,
            content: ElfSectionContent::StringTable(ElfStringTable::new(
                self.section_names.strings.clone(),
            )),
        }
    }

    fn prepare_segments(&self) -> Vec<ElfSegment<BuiltElfIds>> {
        let mut elf_segments = Vec::new();
        for segment in self.layout.iter_segments() {
            elf_segments.push((
                segment.start,
                ElfSegment {
                    type_: ElfSegmentType::Load,
                    perms: segment.perms,
                    content: ElfSegmentContent::Sections(
                        segment
                            .sections
                            .iter()
                            .map(|id| *self.section_ids_mapping.get(id).unwrap())
                            .collect(),
                    ),
                    align: segment.align,
                },
            ));
        }

        // Segments have to be in order in memory, otherwise they will not be loaded.
        elf_segments.sort_by_key(|(addr, _segment)| *addr);
        let mut elf_segments = elf_segments.into_iter().map(|(_a, s)| s).collect::<Vec<_>>();

        // Finally add whether the stack should be executable.
        elf_segments.push(ElfSegment {
            type_: ElfSegmentType::GnuStack,
            perms: ElfPermissions {
                read: true,
                write: true,
                execute: self.object.executable_stack,
            },
            content: ElfSegmentContent::Empty,
            align: 1,
        });

        elf_segments
    }
}

struct PendingStringsTable {
    id: BuiltElfSectionId,
    strings: BTreeMap<u32, String>,
    next_offset: u32,
}

impl PendingStringsTable {
    fn new(ids: &mut BuiltElfIds) -> Self {
        let mut strings = BTreeMap::new();
        strings.insert(0, String::new()); // First string has to always be empty.
        Self { id: ids.allocate_section_id(), strings, next_offset: 1 }
    }

    fn add(&mut self, string: &str) -> BuiltElfStringId {
        let offset = self.next_offset;
        self.next_offset += string.len() as u32 + 1;
        self.strings.insert(offset, string.into());
        BuiltElfStringId::new(self.id, offset)
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
