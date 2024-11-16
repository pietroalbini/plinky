mod dynamic;
mod relocations;
mod strings;
mod symbols;
pub(crate) mod sysv_hash;

use crate::cli::Mode;
use crate::interner::Interned;
use crate::passes::build_elf::dynamic::build_dynamic_section;
use crate::passes::build_elf::relocations::{create_relocations, RelaCreationError};
use crate::passes::build_elf::strings::{create_strings, BuiltStringsTable};
use crate::passes::build_elf::symbols::{create_symbols, BuiltSymbolsTable};
use crate::passes::build_elf::sysv_hash::create_sysv_hash;
use crate::repr::object::Object;
use crate::repr::sections::{SectionContent, SectionId};
use crate::repr::segments::SegmentType;
use crate::repr::symbols::{ResolveSymbolError, ResolvedSymbol};
use crate::utils::address_resolver::AddressResolver;
use plinky_elf::ids::{ElfSectionId, ElfStringId};
use plinky_elf::writer::layout::Layout;
use plinky_elf::{
    ElfNotesTable, ElfObject, ElfProgramSection, ElfSection, ElfSectionContent, ElfSegment,
    ElfSegmentType, ElfStringTable, ElfType, ElfUninitializedSection,
};
use plinky_macros::{Display, Error};
use plinky_utils::ints::{Address, ExtractNumber};
use std::collections::BTreeMap;
use std::num::NonZeroU64;

type SectionConversion = BTreeMap<SectionId, ElfSectionId>;

pub(crate) fn run(
    object: Object,
    layout: &Layout<SectionId>,
    resolver: &AddressResolver<'_>,
) -> Result<(ElfObject, SectionConversion), ElfBuilderError> {
    let builder = ElfBuilder {
        section_zero_id: ElfSectionId { index: 0 },
        section_ids: BTreeMap::new(),

        layout,
        resolver,
        object,
        pending_string_tables: BTreeMap::new(),
        pending_symbol_tables: BTreeMap::new(),
    };
    builder.build()
}

struct ElfBuilder<'a> {
    object: Object,
    layout: &'a Layout<SectionId>,
    resolver: &'a AddressResolver<'a>,

    section_zero_id: ElfSectionId,
    section_ids: BTreeMap<SectionId, ElfSectionId>,

    pending_string_tables: BTreeMap<SectionId, BuiltStringsTable>,
    pending_symbol_tables: BTreeMap<SectionId, BuiltSymbolsTable>,
}

impl<'a> ElfBuilder<'a> {
    fn build(mut self) -> Result<(ElfObject, SectionConversion), ElfBuilderError> {
        // Precalculate section IDs, to avoid circular dependencies.
        for (index, section) in self.object.sections.iter().enumerate() {
            // The +1 is due to the zero section.
            self.section_ids.insert(section.id, ElfSectionId { index: index as u32 + 1 });
        }

        // Precalculate string tables, as other sections will need to reference string IDs.
        for section in self.object.sections.iter() {
            let SectionContent::Strings(strings) = &section.content else { continue };
            let pending = create_strings(&self, section.id, strings);
            self.pending_string_tables.insert(section.id, pending);
        }

        // Precalculate symbol tables, as other sections will need to reference symbol IDs. This
        // has to happen after precalculating string tables.
        for section in self.object.sections.iter() {
            let SectionContent::Symbols(symbols) = &section.content else { continue };
            let pending = create_symbols(
                *self.section_ids.get(&section.id).unwrap(),
                &self.section_ids,
                &self.pending_string_tables,
                &self.object.symbols,
                symbols,
            );
            self.pending_symbol_tables.insert(section.id, pending);
        }

        let entry = self.prepare_entry_point()?;
        let sections = self.prepare_sections()?;
        let segments = self.prepare_segments();

        assert!(self.pending_symbol_tables.values().all(|t| t.elf.is_none()));
        assert!(self.pending_string_tables.values().all(|t| t.elf.is_none()));

        Ok((
            ElfObject {
                env: self.object.env,
                type_: match self.object.mode {
                    Mode::PositionDependent => ElfType::Executable,
                    Mode::PositionIndependent => ElfType::SharedObject,
                    Mode::SharedLibrary => ElfType::SharedObject,
                },
                entry,
                sections,
                segments,
            },
            self.section_ids,
        ))
    }

    fn prepare_entry_point(&self) -> Result<Option<NonZeroU64>, ElfBuilderError> {
        let Some(symbol_id) = self.object.entry_point else { return Ok(None) };
        let symbol = self.object.symbols.get(symbol_id);
        let resolved = symbol
            .resolve(&self.resolver, 0.into())
            .map_err(ElfBuilderError::EntryPointResolution)?;

        match resolved {
            ResolvedSymbol::Absolute(_) => {
                Err(ElfBuilderError::EntryPointNotAnAddress(symbol.name()))
            }
            ResolvedSymbol::Address { memory_address, .. } => Ok(Some(
                NonZeroU64::new(
                    memory_address
                        .extract()
                        .try_into()
                        .map_err(|_| ElfBuilderError::EntrypointIsOutOfBounds(memory_address))?,
                )
                .ok_or(ElfBuilderError::EntrypointIsZero(symbol.name()))?,
            )),
        }
    }

    fn prepare_sections(&mut self) -> Result<BTreeMap<ElfSectionId, ElfSection>, ElfBuilderError> {
        // Prepare section names ahead of time.
        let mut section_names = StringsTableBuilder::new(
            *self
                .section_ids
                .get(
                    &self
                        .object
                        .sections
                        .iter()
                        .filter(|s| matches!(s.content, SectionContent::SectionNames))
                        .map(|s| s.id)
                        .next()
                        .ok_or(ElfBuilderError::NoSectionNamesSection)?,
                )
                .unwrap(),
        );
        let mut section_names_map = BTreeMap::new();
        for section in self.object.sections.iter() {
            section_names_map
                .insert(section.id, section_names.add(section.name.resolve().as_str()));
        }
        let zero_section_name = section_names.zero_id;
        let mut section_names = Some(section_names);

        let mut sections = BTreeMap::new();

        sections.insert(
            self.section_zero_id,
            ElfSection {
                name: zero_section_name,
                memory_address: 0,
                part_of_group: false,
                content: ElfSectionContent::Null,
            },
        );

        while let Some(section) = self.object.sections.pop_first() {
            let content = match &section.content {
                SectionContent::Data(data) => ElfSectionContent::Program(ElfProgramSection {
                    perms: data.perms,
                    deduplication: data.deduplication,
                    raw: data.bytes.clone(),
                }),

                SectionContent::Uninitialized(uninit) => {
                    ElfSectionContent::Uninitialized(ElfUninitializedSection {
                        perms: uninit.perms,
                        len: uninit.len.extract(),
                    })
                }

                SectionContent::Strings(_) => self
                    .pending_string_tables
                    .get_mut(&section.id)
                    .expect("string table should've been prepared")
                    .elf
                    .take()
                    .expect("another section already took the string table"),

                SectionContent::Symbols(_) => self
                    .pending_symbol_tables
                    .get_mut(&section.id)
                    .expect("symbol table should've been prepared")
                    .elf
                    .take()
                    .expect("another section already took the symbol table"),

                SectionContent::SysvHash(sysv) => create_sysv_hash(
                    self.object.symbols.iter(&*sysv.view),
                    *self.section_ids.get(&sysv.symbols).unwrap(),
                ),

                SectionContent::Relocations(relocations) => create_relocations(
                    self.object.relocation_mode(),
                    relocations.section(),
                    relocations.relocations().into_iter(),
                    self.object.env.class,
                    *self.section_ids.get(&relocations.section()).unwrap(),
                    *self.section_ids.get(&relocations.symbols_table()).unwrap(),
                    &self
                        .pending_symbol_tables
                        .get(&relocations.symbols_table())
                        .expect("missing symbol table")
                        .conversion,
                    &self.resolver,
                )?,

                SectionContent::Dynamic(dynamic) => build_dynamic_section(self, dynamic),

                SectionContent::Notes(notes) => {
                    ElfSectionContent::Note(ElfNotesTable { notes: notes.notes.clone() })
                }

                SectionContent::SectionNames => section_names
                    .take()
                    .ok_or(ElfBuilderError::MoreThanOneSectionNamesSection)?
                    .into_elf(),
            };

            sections.insert(
                *self.section_ids.get(&section.id).unwrap(),
                ElfSection {
                    name: *section_names_map.get(&section.id).unwrap(),
                    memory_address: self
                        .layout
                        .metadata_of_section(&section.id)
                        .memory
                        .as_ref()
                        .map(|m| m.address.extract())
                        .unwrap_or(0),
                    part_of_group: false,
                    content,
                },
            );
        }

        Ok(sections)
    }

    fn prepare_segments(&self) -> Vec<ElfSegment> {
        let mut elf_segments = Vec::new();
        for (_id, segment) in self.object.segments.iter() {
            let layout = segment.layout(self.layout);
            elf_segments.push(ElfSegment {
                type_: match segment.type_ {
                    SegmentType::Dynamic => ElfSegmentType::Dynamic,
                    SegmentType::Interpreter => ElfSegmentType::Interpreter,
                    SegmentType::Program => ElfSegmentType::Load,
                    SegmentType::ProgramHeader => ElfSegmentType::ProgramHeaderTable,
                    SegmentType::Uninitialized => ElfSegmentType::Load,
                    SegmentType::GnuStack => ElfSegmentType::GnuStack,
                    SegmentType::GnuRelro => ElfSegmentType::GnuRelro,
                    SegmentType::GnuProperty => ElfSegmentType::GnuProperty,
                    SegmentType::Notes => ElfSegmentType::Note,
                },
                perms: segment.perms,
                align: segment.align,
                file_offset: layout.file.as_ref().map(|f| f.offset.extract() as u64).unwrap_or(0),
                file_size: layout.file.as_ref().map(|f| f.len.extract()).unwrap_or(0),
                virtual_address: layout.memory.as_ref().map(|m| m.address.extract()).unwrap_or(0),
                memory_size: layout.memory.as_ref().map(|m| m.len.extract()).unwrap_or(0),
            });
        }

        // Segments have to be in order in memory, otherwise they will not be loaded.
        elf_segments.sort_by_key(|segment| (segment.type_, segment.virtual_address));
        elf_segments
    }
}

struct StringsTableBuilder {
    id: ElfSectionId,
    strings: BTreeMap<u32, String>,
    next_offset: u32,
    zero_id: ElfStringId,
}

impl StringsTableBuilder {
    fn new(id: ElfSectionId) -> Self {
        let mut strings = BTreeMap::new();
        strings.insert(0, String::new()); // First string has to always be empty.
        Self { id, strings, next_offset: 1, zero_id: ElfStringId { section: id, offset: 0 } }
    }

    fn add(&mut self, string: &str) -> ElfStringId {
        let offset = self.next_offset;
        self.next_offset += string.len() as u32 + 1;
        self.strings.insert(offset, string.into());
        ElfStringId { section: self.id, offset }
    }

    fn into_elf(self) -> ElfSectionContent {
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
    #[display("no section names sections are present")]
    NoSectionNamesSection,
    #[display("more than one section names section is present")]
    MoreThanOneSectionNamesSection,
    #[display("failed to create a relocations section")]
    RelaCreation(#[from] RelaCreationError),
}
