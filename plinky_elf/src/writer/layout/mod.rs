use crate::ids::ElfIds;
use crate::raw::{
    RawGroupFlags, RawHashHeader, RawHeader, RawIdentification, RawProgramHeader, RawRel, RawRela,
    RawSectionHeader, RawSymbol,
};
use crate::{
    ElfClass, ElfObject, ElfSection, ElfSectionContent, ElfSegmentContent, ElfSegmentType,
};
use plinky_macros::{Display, Error};
use plinky_utils::raw_types::{RawType, RawTypeAsPointerSize};
use std::collections::BTreeMap;

const ALIGN: u64 = 0x1000;

#[derive(Debug)]
pub(super) struct WriteLayout<I: ElfIds> {
    parts: Vec<Part<I::SectionId>>,
    metadata: BTreeMap<Part<I::SectionId>, PartMetadata>,
    current_offset: u64,
    pub(super) header_size: u64,
    next_padding_id: usize,
    class: ElfClass,
}

impl<I: ElfIds> WriteLayout<I> {
    pub(super) fn new(object: &ElfObject<I>) -> Result<Self, WriteLayoutError> {
        let mut layout = WriteLayout {
            parts: Vec::new(),
            metadata: BTreeMap::new(),
            current_offset: 0,
            header_size: 0,
            next_padding_id: 0,
            class: object.env.class,
        };

        layout.add_part(Part::Identification, RawIdentification::size(layout.class));
        layout.add_part(Part::Header, RawHeader::size(layout.class));
        layout.header_size = layout.current_offset;

        let sections_in_load_segments = object
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
        for (id, section) in &object.sections {
            if let Some(segment) = sections_in_load_segments.get(&id) {
                put_in_segments.entry(*segment).or_insert_with(Vec::new).push((id, section));
            } else {
                put_in_preamble.push((id, section));
            }
        }
        for (id, section) in put_in_preamble {
            layout.add_section(id, section)?;
        }
        for segment_sections in put_in_segments.values() {
            layout.align_to_page();
            for (id, section) in segment_sections {
                layout.add_section(id, section)?;
            }
        }

        // TODO: waste less space, and try to put the program header next to the elf header.
        layout.align_to_page();
        layout.add_part(
            Part::ProgramHeaders,
            RawProgramHeader::size(layout.class) * object.segments.len(),
        );

        layout.add_part(
            Part::SectionHeaders,
            RawSectionHeader::size(layout.class) * object.sections.len(),
        );

        Ok(layout)
    }

    fn add_section(
        &mut self,
        id: &I::SectionId,
        section: &ElfSection<I>,
    ) -> Result<(), WriteLayoutError> {
        match &section.content {
            ElfSectionContent::Null => {}
            ElfSectionContent::Program(program) => {
                self.add_part(Part::ProgramSection(id.clone()), program.raw.len())
            }
            ElfSectionContent::Uninitialized(_) => {
                // Uninitialized sections are not part of the file layout.
            }
            ElfSectionContent::SymbolTable(table) => self.add_part(
                Part::SymbolTable(id.clone()),
                RawSymbol::size(self.class) * table.symbols.len(),
            ),
            ElfSectionContent::StringTable(table) => {
                self.add_part(Part::StringTable(id.clone()), table.len());
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
                self.add_part(
                    Part::RelocationsTable { id: id.clone(), rela },
                    if rela {
                        (RawRela::size(self.class) * table.relocations.len()) as _
                    } else {
                        (RawRel::size(self.class) * table.relocations.len()) as _
                    },
                )
            }
            ElfSectionContent::Group(group) => {
                self.add_part(
                    Part::Group(id.clone()),
                    RawGroupFlags::size(self.class) + u32::size(self.class) * group.sections.len(),
                );
            }
            ElfSectionContent::Hash(hash) => {
                let size = u32::size(self.class);
                self.add_part(
                    Part::Hash(id.clone()),
                    RawHashHeader::size(self.class)
                        + hash.buckets.len() * size
                        + hash.chain.len() * size,
                )
            }
            ElfSectionContent::Dynamic(dynamic) => {
                let size = <u64 as RawTypeAsPointerSize>::size(self.class) * 2;
                self.add_part(Part::Dynamic(id.clone()), dynamic.directives.len() * size);
            }
            ElfSectionContent::Note(_) => {
                return Err(WriteLayoutError::WritingNotesUnsupported);
            }
            ElfSectionContent::Unknown(_) => {
                return Err(WriteLayoutError::UnknownSection);
            }
        }
        Ok(())
    }

    fn add_part(&mut self, part: Part<I::SectionId>, len: usize) {
        let len = len as u64;
        self.parts.push(part.clone());
        self.metadata.insert(part, PartMetadata { len, offset: self.current_offset });
        self.current_offset += len;
    }

    fn align_to_page(&mut self) {
        let len = self.len();
        if len % ALIGN == 0 {
            return;
        }
        let bytes_to_pad = ALIGN - len % ALIGN;
        self.add_part(Part::Padding(PaddingId(self.next_padding_id)), bytes_to_pad as _);
        self.next_padding_id += 1;
    }

    fn len(&self) -> u64 {
        self.current_offset
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
                Part::Identification => false,
                Part::Header => false,
                Part::SectionHeaders => false,
                Part::ProgramHeaders => false,
                Part::ProgramSection(this) => this == id,
                Part::StringTable(this) => this == id,
                Part::SymbolTable(this) => this == id,
                Part::Padding(_) => false,
                Part::Group(this) => this == id,
                Part::Hash(this) => this == id,
                Part::Dynamic(this) => this == id,
                Part::RelocationsTable { id: this, .. } => this == id,
            })
            .map(|(_, value)| value)
            .next()
            .unwrap()
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub(super) enum Part<SectionId> {
    Identification,
    Header,
    SectionHeaders,
    ProgramHeaders,
    ProgramSection(SectionId),
    StringTable(SectionId),
    SymbolTable(SectionId),
    Hash(SectionId),
    RelocationsTable { id: SectionId, rela: bool },
    Group(SectionId),
    Dynamic(SectionId),
    Padding(PaddingId),
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
