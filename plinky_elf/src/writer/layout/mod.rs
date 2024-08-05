mod details_provider;

use crate::ids::ElfIds;
use crate::raw::{
    RawGroupFlags, RawHashHeader, RawHeader, RawIdentification, RawProgramHeader, RawRel, RawRela,
    RawSectionHeader, RawSymbol,
};
use crate::writer::layout::details_provider::LayoutDetailsProvider;
use crate::{ElfSection, ElfSectionContent, ElfSegmentContent, ElfSegmentType};
use plinky_macros::{Display, Error};
use plinky_utils::raw_types::{RawType, RawTypeAsPointerSize};
use std::collections::BTreeMap;

const ALIGN: u64 = 0x1000;

#[derive(Debug)]
pub(super) struct WriteLayout<I: ElfIds> {
    parts: Vec<Part<I::SectionId>>,
    metadata: BTreeMap<Part<I::SectionId>, PartMetadata>,
}

impl<I: ElfIds> WriteLayout<I> {
    pub(super) fn new(details: &dyn LayoutDetailsProvider<I>) -> Result<Self, WriteLayoutError> {
        let builder = LayoutBuilder {
            details,
            layout: WriteLayout { parts: Vec::new(), metadata: BTreeMap::new() },
            current_offset: 0,
            next_padding_id: 0,
        };
        builder.build()
    }

    pub(super) fn parts(&self) -> &[Part<I::SectionId>] {
        &self.parts
    }

    pub(super) fn metadata(&self, part: &Part<I::SectionId>) -> &PartMetadata {
        self.metadata.get(part).unwrap()
    }

    pub(super) fn metadata_of_section(&self, id: &I::SectionId) -> &PartMetadata {
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
    layout: WriteLayout<I>,
    current_offset: u64,
    next_padding_id: usize,
}

impl<I: ElfIds> LayoutBuilder<'_, I> {
    fn build(mut self) -> Result<WriteLayout<I>, WriteLayoutError> {
        self.add_part(Part::Header);

        let sections_in_load_segments = self
            .details
            .object()
            .segments
            .iter()
            .enumerate()
            .filter(|(_idx, s)| matches!(s.type_, ElfSegmentType::Load))
            .flat_map(|(idx, s)| match &s.content {
                ElfSegmentContent::Empty => {
                    Box::new(std::iter::empty()) as Box<dyn Iterator<Item = _>>
                }
                ElfSegmentContent::ElfHeader => Box::new(std::iter::empty()),
                ElfSegmentContent::ProgramHeader => Box::new(std::iter::empty()),
                ElfSegmentContent::Sections(s) => Box::new(s.iter().map(move |s| (s, idx))),
                ElfSegmentContent::Unknown(_) => Box::new(std::iter::empty()),
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
        for (id, section) in &self.details.object().sections {
            if let Some(segment) = sections_in_load_segments.get(&id) {
                put_in_segments.entry(*segment).or_insert_with(Vec::new).push((id, section));
            } else {
                put_in_preamble.push((id, section));
            }
        }
        for (id, section) in put_in_preamble {
            self.add_section(id, section)?;
        }
        for segment_sections in put_in_segments.values() {
            self.align_to_page();
            for (id, section) in segment_sections {
                self.add_section(id, section)?;
            }
        }

        // TODO: waste less space, and try to put the program header next to the elf header.
        self.align_to_page();
        self.add_part(Part::ProgramHeaders);
        self.add_part(Part::SectionHeaders);

        Ok(self.layout)
    }

    fn add_section(
        &mut self,
        id: &I::SectionId,
        section: &ElfSection<I>,
    ) -> Result<(), WriteLayoutError> {
        match &section.content {
            ElfSectionContent::Null => {}
            ElfSectionContent::Program(_) => {
                self.add_part(Part::ProgramSection(id.clone()));
            }
            ElfSectionContent::Uninitialized(_) => {
                // Uninitialized sections are not part of the file layout.
            }
            ElfSectionContent::SymbolTable(_) => {
                self.add_part(Part::SymbolTable(id.clone()));
            }
            ElfSectionContent::StringTable(_) => {
                self.add_part(Part::StringTable(id.clone()));
            }

            ElfSectionContent::RelocationsTable(table) => {
                let mut rela = None;
                for relocation in &table.relocations {
                    match rela {
                        Some(rela) if rela == relocation.addend.is_some() => {}
                        Some(_) => return Err(WriteLayoutError::MixedRelRela),
                        None => rela = Some(relocation.addend.is_some()),
                    }
                }
                let rela = rela.unwrap_or(false);
                if rela {
                    self.add_part(Part::Rela(id.clone()));
                } else {
                    self.add_part(Part::Rel(id.clone()));
                }
            }
            ElfSectionContent::Group(_) => self.add_part(Part::Group(id.clone())),
            ElfSectionContent::Hash(_) => self.add_part(Part::Hash(id.clone())),
            ElfSectionContent::Dynamic(_) => self.add_part(Part::Dynamic(id.clone())),

            ElfSectionContent::Note(_) => {
                return Err(WriteLayoutError::WritingNotesUnsupported);
            }
            ElfSectionContent::Unknown(_) => {
                return Err(WriteLayoutError::UnknownSection);
            }
        }
        Ok(())
    }

    fn add_part(&mut self, part: Part<I::SectionId>) {
        let len = part_len(self.details, &part) as u64;
        self.layout.parts.push(part.clone());
        self.layout.metadata.insert(part, PartMetadata { len, offset: self.current_offset });
        self.current_offset += len;
    }

    fn align_to_page(&mut self) {
        let len = self.len();
        if len % ALIGN == 0 {
            return;
        }
        let bytes_to_pad = ALIGN - len % ALIGN;
        self.add_part(Part::Padding {
            id: PaddingId(self.next_padding_id),
            len: bytes_to_pad as _,
        });
        self.next_padding_id += 1;
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
pub(super) enum Part<SectionId> {
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
pub(super) struct PaddingId(usize);

#[derive(Debug)]
pub(super) struct PartMetadata {
    pub(super) len: u64,
    pub(super) offset: u64,
}

#[derive(Debug, Error, Display)]
pub enum WriteLayoutError {
    #[display("relocation section mixing rel and rela")]
    MixedRelRela,
    #[display("writing notes is not supported yet")]
    WritingNotesUnsupported,
    #[display("unkown section encountered while calculating the layout")]
    UnknownSection,
}
