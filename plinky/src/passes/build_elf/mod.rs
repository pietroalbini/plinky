mod dynamic;
pub(crate) mod ids;
mod relocations;
mod sections;
mod symbols;

use crate::cli::Mode;
use crate::interner::Interned;
use crate::passes::build_elf::ids::{BuiltElfIds, BuiltElfSectionId, BuiltElfStringId};
use crate::passes::build_elf::sections::Sections;
use crate::passes::build_elf::symbols::create_symbols;
use crate::passes::layout::Layout;
use crate::repr::object::Object;
use crate::repr::sections::SectionContent;
use crate::repr::segments::{SegmentContent, SegmentType};
use crate::repr::symbols::views::AllSymbols;
use crate::repr::symbols::{ResolveSymbolError, ResolvedSymbol};
use crate::utils::ints::{Address, ExtractNumber};
use plinky_elf::ids::serial::SerialIds;
use plinky_elf::{
    ElfObject, ElfPermissions, ElfProgramSection, ElfSectionContent, ElfSegment, ElfSegmentContent,
    ElfSegmentType, ElfStringTable, ElfType, ElfUninitializedSection, RawBytes,
};
use plinky_macros::{Display, Error};
use std::collections::BTreeMap;
use std::num::NonZeroU64;

pub(crate) fn run(
    object: Object,
    layout: Layout,
    old_ids: SerialIds,
) -> Result<ElfObject<BuiltElfIds>, ElfBuilderError> {
    let mut ids = BuiltElfIds::new();
    let builder = ElfBuilder { object, layout, sections: Sections::new(&mut ids), ids, old_ids };
    builder.build()
}

struct ElfBuilder {
    object: Object,
    layout: Layout,
    sections: Sections,
    ids: BuiltElfIds,
    old_ids: SerialIds,
}

impl ElfBuilder {
    fn build(mut self) -> Result<ElfObject<BuiltElfIds>, ElfBuilderError> {
        let entry = self.prepare_entry_point()?;
        self.prepare_sections();

        match self.object.mode {
            Mode::PositionDependent => {}
            Mode::PositionIndependent => dynamic::add(&mut self),
        }

        let symbols = create_symbols(
            &self.object.symbols,
            &AllSymbols,
            &mut self.ids,
            &mut self.sections,
        );
        self.sections.create(".symtab", symbols.symbol_table).add(&mut self.ids);
        self.sections.create(".strtab", symbols.string_table).add_with_id(symbols.string_table_id);

        let segments = self.prepare_segments();

        Ok(ElfObject {
            env: self.object.env,
            type_: match self.object.mode {
                Mode::PositionDependent => ElfType::Executable,
                Mode::PositionIndependent => ElfType::SharedObject,
            },
            entry,
            sections: self.sections.finalize(),
            segments,
        })
    }

    fn prepare_entry_point(&self) -> Result<Option<NonZeroU64>, ElfBuilderError> {
        let symbol = self.object.symbols.get(self.object.entry_point);
        let resolved = symbol
            .resolve(&self.layout, 0.into())
            .map_err(ElfBuilderError::EntryPointResolution)?;

        match resolved {
            ResolvedSymbol::Absolute(_) => {
                Err(ElfBuilderError::EntryPointNotAnAddress(symbol.name))
            }
            ResolvedSymbol::Address { memory_address, .. } => Ok(Some(
                NonZeroU64::new(
                    memory_address
                        .extract()
                        .try_into()
                        .map_err(|_| ElfBuilderError::EntrypointIsOutOfBounds(memory_address))?,
                )
                .ok_or(ElfBuilderError::EntrypointIsZero(symbol.name))?,
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
                                perms: data.perms,
                                deduplication: data.deduplication,
                                raw: RawBytes(data.bytes.clone()),
                            }),
                        )
                        .layout(self.layout.of_section(section.id))
                        .old_id(section.id)
                        .add(&mut self.ids);
                }
                SectionContent::Uninitialized(uninit) => {
                    self.sections
                        .create(
                            &section.name.resolve(),
                            ElfSectionContent::Uninitialized(ElfUninitializedSection {
                                perms: uninit.perms,
                                len: uninit.len,
                            }),
                        )
                        .layout(self.layout.of_section(section.id))
                        .old_id(section.id)
                        .add(&mut self.ids);
                }
            }
        }
    }

    fn prepare_segments(&self) -> Vec<ElfSegment<BuiltElfIds>> {
        let mut elf_segments = Vec::new();
        for segment in self.object.segments.iter() {
            elf_segments.push((
                segment.start,
                ElfSegment {
                    type_: match segment.type_ {
                        SegmentType::Dynamic => ElfSegmentType::Dynamic,
                        SegmentType::Interpreter => ElfSegmentType::Interpreter,
                        SegmentType::Program => ElfSegmentType::Load,
                        SegmentType::ProgramHeader => ElfSegmentType::ProgramHeaderTable,
                        SegmentType::Uninitialized => ElfSegmentType::Load,
                    },
                    perms: segment.perms,
                    content: match &segment.content {
                        SegmentContent::ElfHeader => ElfSegmentContent::ElfHeader,
                        SegmentContent::ProgramHeader => ElfSegmentContent::ProgramHeader,
                        SegmentContent::Sections(sections) => ElfSegmentContent::Sections(
                            sections.iter().map(|id| self.sections.new_id_of(*id)).collect(),
                        ),
                    },
                    align: segment.align,
                },
            ));
        }

        // Segments have to be in order in memory, otherwise they will not be loaded.
        elf_segments.sort_by_key(|(addr, segment)| (segment.type_, *addr));
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
    #[display("the entry point address {f0} is out of bounds")]
    EntrypointIsOutOfBounds(Address),
}
