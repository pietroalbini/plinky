pub(crate) mod ids;
mod sections;

use crate::interner::Interned;
use crate::passes::build_elf::ids::{BuiltElfIds, BuiltElfSectionId, BuiltElfStringId};
use crate::passes::build_elf::sections::Sections;
use crate::passes::layout::Layout;
use crate::repr::object::Object;
use crate::repr::sections::SectionContent;
use crate::repr::symbols::{ResolveSymbolError, ResolvedSymbol};
use plinky_elf::{
    ElfObject, ElfPermissions, ElfProgramSection, ElfSectionContent, ElfSegment, ElfSegmentContent,
    ElfSegmentType, ElfStringTable, ElfType, ElfUninitializedSection, RawBytes,
};
use plinky_macros::{Display, Error};
use std::collections::BTreeMap;
use std::num::NonZeroU64;

pub(crate) fn run(
    object: Object,
    layout: &Layout,
) -> Result<ElfObject<BuiltElfIds>, ElfBuilderError> {
    let mut ids = BuiltElfIds::new();
    let builder = ElfBuilder { object, layout, sections: Sections::new(&mut ids), ids };
    builder.build()
}

struct ElfBuilder<'a> {
    object: Object,
    layout: &'a Layout,
    sections: Sections,
    ids: BuiltElfIds,
}

impl ElfBuilder<'_> {
    fn build(mut self) -> Result<ElfObject<BuiltElfIds>, ElfBuilderError> {
        let entry = self.prepare_entry_point()?;
        self.prepare_sections();
        let segments = self.prepare_segments();

        Ok(ElfObject {
            env: self.object.env,
            type_: ElfType::Executable,
            entry,
            sections: self.sections.finalize(),
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

    fn prepare_sections(&mut self) {
        while let Some(section) = self.object.sections.pop_first() {
            match &section.content {
                SectionContent::Data(data) => {
                    self.sections
                        .create(
                            &section.name.resolve(),
                            ElfSectionContent::Program(ElfProgramSection {
                                perms: section.perms,
                                deduplication: data.deduplication,
                                raw: RawBytes(data.bytes.clone()),
                            }),
                        )
                        .layout(self.layout.of_section(section.id))
                        .old_id(section.id)
                        .add(&mut self.ids);
                }
                SectionContent::Uninitialized(uninit) => self
                    .sections
                    .create(
                        &section.name.resolve(),
                        ElfSectionContent::Uninitialized(ElfUninitializedSection {
                            perms: section.perms,
                            len: uninit.len,
                        }),
                    )
                    .layout(self.layout.of_section(section.id))
                    .old_id(section.id)
                    .add(&mut self.ids),
            }
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
                        segment.sections.iter().map(|id| self.sections.new_id_of(*id)).collect(),
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
    zero_id: BuiltElfStringId,
}

impl PendingStringsTable {
    fn new(ids: &mut BuiltElfIds) -> Self {
        let id = ids.allocate_section_id();
        let mut strings = BTreeMap::new();
        strings.insert(0, String::new()); // First string has to always be empty.
        Self { id, strings, next_offset: 1, zero_id: BuiltElfStringId::new(id, 0) }
    }

    fn add(&mut self, string: &str) -> BuiltElfStringId {
        let offset = self.next_offset;
        self.next_offset += string.len() as u32 + 1;
        self.strings.insert(offset, string.into());
        BuiltElfStringId::new(self.id, offset)
    }

    fn into_elf(self) -> ElfSectionContent<BuiltElfIds> {
        ElfSectionContent::StringTable(ElfStringTable::new(self.strings))
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
