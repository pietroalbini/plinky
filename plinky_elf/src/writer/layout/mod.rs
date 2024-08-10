mod details_provider;

use crate::ids::ElfIds;
use crate::raw::{
    RawGroupFlags, RawHashHeader, RawHeader, RawIdentification, RawProgramHeader, RawRel, RawRela,
    RawSectionHeader, RawSymbol,
};
use crate::writer::layout::details_provider::LayoutDetailsProvider;
use plinky_macros::{Display, Error};
use plinky_utils::raw_types::{RawType, RawTypeAsPointerSize};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};

const ALIGN: u64 = 0x1000;

#[derive(Debug)]
pub struct Layout<I: ElfIds> {
    parts: Vec<Part<I::SectionId>>,
    metadata: BTreeMap<Part<I::SectionId>, PartMetadata>,
}

impl<I: ElfIds> Layout<I> {
    pub fn new(details: &dyn LayoutDetailsProvider<I>) -> Result<Self, LayoutError> {
        let builder = LayoutBuilder {
            details,
            layout: Layout { parts: Vec::new(), metadata: BTreeMap::new() },
            current_offset: 0,
        };
        builder.build()
    }

    pub fn parts(&self) -> &[Part<I::SectionId>] {
        &self.parts
    }

    pub fn metadata(&self, part: &Part<I::SectionId>) -> &PartMetadata {
        self.metadata.get(part).unwrap()
    }

    pub fn metadata_of_section(&self, id: &I::SectionId) -> &PartMetadata {
        self.metadata
            .iter()
            .filter(|(key, _)| match key {
                Part::Header => false,
                Part::SectionHeaders => false,
                Part::ProgramHeaders => false,
                Part::ProgramSection(this) => this == id,
                Part::StringTable(this) => this == id,
                Part::SymbolTable(this) => this == id,
                Part::Padding { .. } => false,
                Part::Group(this) => this == id,
                Part::Hash(this) => this == id,
                Part::Dynamic(this) => this == id,
                Part::Rel(this) => this == id,
                Part::Rela(this) => this == id,
            })
            .map(|(_, value)| value)
            .next()
            .unwrap()
    }
}

struct LayoutBuilder<'a, I: ElfIds> {
    details: &'a dyn LayoutDetailsProvider<I>,
    layout: Layout<I>,
    current_offset: u64,
}

impl<I: ElfIds> LayoutBuilder<'_, I> {
    fn build(mut self) -> Result<Layout<I>, LayoutError> {
        self.add_part(Part::Header);

        let sections_in_load_segments = self
            .details
            .loadable_segments()
            .into_iter()
            .enumerate()
            .flat_map(|(idx, segment)| {
                segment.sections.into_iter().map(move |section| (section, idx))
            })
            .collect::<BTreeMap<_, _>>();

        // We need to be careful in how sections are written in the resulting ELF file: each LOAD
        // segment *as a whole* needs to be page aligned, while sections within the segment need to
        // be contiguous. To address that, we split sections into sections that are not part of any
        // segment and thus can be written adjacent to each other at the start of the ELF (the
        // "preamble"), and sections that belong to a segment.
        //
        // We then proceed to write all the preamble sections, and after that write all the
        // segments while being careful of page-aligning each of them.
        let mut put_in_preamble = Vec::new();
        let mut put_in_segments = BTreeMap::new();
        for (id, part) in self.details.parts_for_sections()? {
            if let Some(segment) = sections_in_load_segments.get(&id) {
                put_in_segments.entry(*segment).or_insert_with(Vec::new).push(part);
            } else {
                put_in_preamble.push(part);
            }
        }
        for part in put_in_preamble {
            self.add_part(part);
        }
        for segment_sections in put_in_segments.into_values() {
            self.align_to_page();
            for part in segment_sections {
                self.add_part(part);
            }
        }

        // TODO: waste less space, and try to put the program header next to the elf header.
        self.align_to_page();
        self.add_part(Part::ProgramHeaders);
        self.add_part(Part::SectionHeaders);

        Ok(self.layout)
    }

    fn add_part(&mut self, part: Part<I::SectionId>) {
        let len = part_len(self.details, &part) as u64;
        self.layout.parts.push(part.clone());
        self.layout.metadata.insert(
            part,
            PartMetadata { file: Some(PartFile { len, offset: self.current_offset }), memory: None },
        );
        self.current_offset += len;
    }

    fn align_to_page(&mut self) {
        let len = self.len();
        if len % ALIGN == 0 {
            return;
        }
        let bytes_to_pad = ALIGN - len % ALIGN;
        self.add_part(Part::Padding { id: PaddingId::next(), len: bytes_to_pad as _ });
    }

    fn len(&self) -> u64 {
        self.current_offset
    }
}

fn part_len<I: ElfIds>(details: &dyn LayoutDetailsProvider<I>, part: &Part<I::SectionId>) -> usize {
    let class = details.class();
    match part {
        Part::Header => RawIdentification::size(class) + RawHeader::size(class),

        Part::SectionHeaders => RawSectionHeader::size(class) * details.sections_count(),
        Part::ProgramHeaders => RawProgramHeader::size(class) * details.segments_count(),

        Part::ProgramSection(id) => details.program_section_len(id),
        Part::StringTable(id) => details.string_table_len(id),

        Part::SymbolTable(id) => RawSymbol::size(class) * details.symbols_in_table_count(id),
        Part::Rel(id) => RawRel::size(class) * details.relocations_in_table_count(id),
        Part::Rela(id) => RawRela::size(class) * details.relocations_in_table_count(id),

        Part::Hash(id) => {
            let hash = details.hash_details(id);
            RawHashHeader::size(class)
                + hash.buckets * u32::size(class)
                + hash.chain * u32::size(class)
        }

        Part::Group(id) => {
            RawGroupFlags::size(class) + u32::size(class) * details.sections_in_group_count(id)
        }
        Part::Dynamic(id) => {
            <u64 as RawTypeAsPointerSize>::size(class) * 2 * details.dynamic_directives_count(id)
        }

        Part::Padding { len, .. } => *len,
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub enum Part<SectionId> {
    Header,
    SectionHeaders,
    ProgramHeaders,
    ProgramSection(SectionId),
    StringTable(SectionId),
    SymbolTable(SectionId),
    Hash(SectionId),
    Rel(SectionId),
    Rela(SectionId),
    Group(SectionId),
    Dynamic(SectionId),
    Padding { id: PaddingId, len: usize },
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub struct PaddingId(u64);

impl PaddingId {
    pub fn next() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        PaddingId(COUNTER.fetch_add(1, Ordering::Relaxed))
    }
}

#[derive(Debug)]
pub struct PartMetadata {
    pub file: Option<PartFile>,
    pub memory: Option<()>,
}

impl PartMetadata {
    pub(super) fn segment_bounds(&self) -> (u64, u64, u64, u64) {
        let (file_offset, file_len) = match &self.file {
            Some(file) => (file.offset, file.len),
            None => (0, 0),
        };

        let (memory_offset, memory_len) = match self.memory {
            Some(()) => (self.file.as_ref().unwrap().offset, self.file.as_ref().unwrap().len),
            None => (0, 0),
        };

        (file_offset, file_len, memory_offset, memory_len)
    }
}

#[derive(Debug)]
pub struct PartFile {
    pub len: u64,
    pub offset: u64,
}

#[derive(Debug, Error, Display)]
pub enum LayoutError {
    #[display("relocation section mixing rel and rela")]
    MixedRelRela,
    #[display("writing notes is not supported yet")]
    WritingNotesUnsupported,
    #[display("unkown section encountered while calculating the layout")]
    UnknownSection,
}
