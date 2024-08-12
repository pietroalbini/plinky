mod dynamic;
pub(crate) mod ids;
mod relocations;
mod symbols;
pub(crate) mod sysv_hash;

use crate::cli::Mode;
use crate::interner::Interned;
use crate::passes::build_elf::dynamic::build_dynamic_section;
use crate::passes::build_elf::ids::{
    BuiltElfIds, BuiltElfSectionId, BuiltElfStringId, BuiltElfSymbolId,
};
use crate::passes::build_elf::relocations::{create_rela, RelaCreationError};
use crate::passes::build_elf::symbols::create_symbols;
use crate::passes::build_elf::sysv_hash::create_sysv_hash;
use crate::repr::object::Object;
use crate::repr::sections::SectionContent;
use crate::repr::segments::SegmentType;
use crate::repr::symbols::{ResolveSymbolError, ResolvedSymbol};
use crate::utils::address_resolver::AddressResolver;
use plinky_elf::ids::serial::{SectionId, SerialIds, SymbolId};
use plinky_elf::writer::layout::Layout;
use plinky_elf::{
    ElfObject, ElfProgramSection, ElfSection, ElfSectionContent, ElfSegment, ElfSegmentType,
    ElfStringTable, ElfType, ElfUninitializedSection, RawBytes,
};
use plinky_macros::{Display, Error};
use plinky_utils::ints::{Address, ExtractNumber};
use std::collections::BTreeMap;
use std::num::NonZeroU64;

type SectionConversion = BTreeMap<SectionId, BuiltElfSectionId>;

pub(crate) fn run(
    object: Object,
    layout: &Layout<SerialIds>,
    resolver: &AddressResolver<'_>,
) -> Result<(ElfObject<BuiltElfIds>, SectionConversion), ElfBuilderError> {
    let mut ids = BuiltElfIds::new();
    let builder = ElfBuilder {
        section_zero_id: ids.allocate_section_id(),
        section_ids: BTreeMap::new(),

        layout,
        resolver,
        object,
        ids,
        pending_symbol_tables: BTreeMap::new(),
        pending_string_tables: BTreeMap::new(),
        symbol_conversion: BTreeMap::new(),
    };
    builder.build()
}

struct ElfBuilder<'a> {
    object: Object,
    layout: &'a Layout<SerialIds>,
    resolver: &'a AddressResolver<'a>,
    ids: BuiltElfIds,

    section_zero_id: BuiltElfSectionId,
    section_ids: BTreeMap<SectionId, BuiltElfSectionId>,

    pending_symbol_tables: BTreeMap<SectionId, ElfSectionContent<BuiltElfIds>>,
    pending_string_tables: BTreeMap<SectionId, ElfSectionContent<BuiltElfIds>>,
    symbol_conversion: BTreeMap<SectionId, BTreeMap<SymbolId, BuiltElfSymbolId>>,
}

impl<'a> ElfBuilder<'a> {
    fn build(mut self) -> Result<(ElfObject<BuiltElfIds>, SectionConversion), ElfBuilderError> {
        // Precalculate section IDs, to avoid circular dependencies.
        for section in self.object.sections.iter() {
            self.section_ids.insert(section.id, self.ids.allocate_section_id());
        }

        // Symbol and string table sections need to be created together (as the string table
        // contains the symbol names for the symbol table), which makes it hard to create them
        // individually as part of prepare_sections(). We thus create them together, and store them
        // in a pending state.
        for section in self.object.sections.iter() {
            let SectionContent::Symbols(symbols_section) = &section.content else { continue };

            let string_table_id = *self.section_ids.get(&symbols_section.strings).unwrap();
            let created = create_symbols(
                &self.object.symbols,
                &*symbols_section.view,
                &mut self.ids,
                &mut self.section_ids,
                string_table_id,
                symbols_section.is_dynamic,
            );
            self.pending_symbol_tables.insert(section.id, created.symbol_table);
            self.pending_string_tables.insert(symbols_section.strings, created.string_table);
            self.symbol_conversion.insert(section.id, created.conversion);
        }

        let entry = self.prepare_entry_point()?;
        let sections = self.prepare_sections()?;

        let segments = self.prepare_segments();

        assert!(self.pending_symbol_tables.is_empty());
        assert!(self.pending_string_tables.is_empty());

        Ok((
            ElfObject {
                env: self.object.env,
                type_: match self.object.mode {
                    Mode::PositionDependent => ElfType::Executable,
                    Mode::PositionIndependent => ElfType::SharedObject,
                },
                entry,
                sections,
                segments,
            },
            self.section_ids,
        ))
    }

    fn prepare_entry_point(&self) -> Result<Option<NonZeroU64>, ElfBuilderError> {
        let symbol = self.object.symbols.get(self.object.entry_point);
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

    fn prepare_sections(
        &mut self,
    ) -> Result<BTreeMap<BuiltElfSectionId, ElfSection<BuiltElfIds>>, ElfBuilderError> {
        // Prepare section names ahead of time.
        let mut section_names = PendingStringsTable::new(
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
                    raw: RawBytes(data.bytes.clone()),
                }),

                SectionContent::Uninitialized(uninit) => {
                    ElfSectionContent::Uninitialized(ElfUninitializedSection {
                        perms: uninit.perms,
                        len: uninit.len.extract(),
                    })
                }

                SectionContent::StringsForSymbols(_) => self
                    .pending_string_tables
                    .remove(&section.id)
                    .expect("string table should've been prepared"),

                SectionContent::Symbols(_) => self
                    .pending_symbol_tables
                    .remove(&section.id)
                    .expect("symbol table should've been prepared"),

                SectionContent::SysvHash(sysv) => create_sysv_hash(
                    self.object.symbols.iter(&*sysv.view).map(|(_id, sym)| sym),
                    *self.section_ids.get(&sysv.symbols).unwrap(),
                ),

                SectionContent::Relocations(relocations) => create_rela(
                    relocations.relocations().into_iter(),
                    self.object.env.class,
                    relocations
                        .section()
                        .map(|s| *self.section_ids.get(&s).unwrap())
                        .unwrap_or(self.section_zero_id),
                    *self.section_ids.get(&relocations.symbols_table()).unwrap(),
                    self.symbol_conversion.get(&relocations.symbols_table()).unwrap(),
                    &self.resolver,
                )?,

                SectionContent::Dynamic(dynamic) => build_dynamic_section(self, dynamic),

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
        for segment in self.object.segments.iter() {
            let layout = segment.layout(self.layout);
            elf_segments.push(ElfSegment {
                type_: match segment.type_ {
                    SegmentType::Dynamic => ElfSegmentType::Dynamic,
                    SegmentType::Interpreter => ElfSegmentType::Interpreter,
                    SegmentType::Program => ElfSegmentType::Load,
                    SegmentType::ProgramHeader => ElfSegmentType::ProgramHeaderTable,
                    SegmentType::Uninitialized => ElfSegmentType::Load,
                    SegmentType::GnuStack => ElfSegmentType::GnuStack,
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

struct PendingStringsTable {
    id: BuiltElfSectionId,
    strings: BTreeMap<u32, String>,
    next_offset: u32,
    zero_id: BuiltElfStringId,
}

impl PendingStringsTable {
    fn new(id: BuiltElfSectionId) -> Self {
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
    #[display("no section names sections are present")]
    NoSectionNamesSection,
    #[display("more than one section names section is present")]
    MoreThanOneSectionNamesSection,
    #[display("failed to create a relocations section")]
    RelaCreation(#[from] RelaCreationError),
}
