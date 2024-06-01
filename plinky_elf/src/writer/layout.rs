use crate::ids::ElfIds;
use crate::raw::{
    RawGroupFlags, RawHashHeader, RawHeader, RawIdentification, RawProgramHeader, RawRel, RawRela,
    RawSectionHeader, RawSymbol,
};
use crate::{ElfClass, ElfObject, ElfSectionContent, ElfSegmentContent};
use plinky_macros::{Display, Error};
use plinky_utils::raw_types::{RawType, RawTypeAsPointerSize};
use std::collections::BTreeMap;

const ALIGN: u64 = 0x1000;

#[derive(Debug)]
pub(super) struct WriteLayout<I: ElfIds> {
    parts: Vec<Part<I::SectionId>>,
    metadata: BTreeMap<Part<I::SectionId>, PartMetadata>,
    current_offset: u64,
    next_padding_id: usize,
    class: ElfClass,
}

impl<I: ElfIds> WriteLayout<I> {
    pub(super) fn new(object: &ElfObject<I>) -> Result<Self, WriteLayoutError> {
        let mut layout = WriteLayout {
            parts: Vec::new(),
            metadata: BTreeMap::new(),
            current_offset: 0,
            next_padding_id: 0,
            class: object.env.class,
        };

        layout.add_part(Part::Identification, RawIdentification::size(layout.class));
        layout.add_part(Part::Header, RawHeader::size(layout.class));
        layout.add_part(
            Part::SectionHeaders,
            RawSectionHeader::size(layout.class) * object.sections.len(),
        );
        layout.add_part(
            Part::ProgramHeaders,
            RawProgramHeader::size(layout.class) * object.segments.len(),
        );

        let mut deferred_program_sections: BTreeMap<I::SectionId, _> = BTreeMap::new();
        for (id, section) in &object.sections {
            match &section.content {
                ElfSectionContent::Null => {}
                ElfSectionContent::Program(program) => {
                    deferred_program_sections
                        .insert(id.clone(), DeferredProgramSection { len: program.raw.len() });
                }
                ElfSectionContent::Uninitialized(_) => {
                    // Uninitialized sections are not part of the file layout.
                }
                ElfSectionContent::SymbolTable(table) => layout.add_part(
                    Part::SymbolTable(id.clone()),
                    RawSymbol::size(layout.class) * table.symbols.len(),
                ),
                ElfSectionContent::StringTable(table) => {
                    layout.add_part(Part::StringTable(id.clone()), table.len());
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
                    layout.add_part(
                        Part::RelocationsTable { id: id.clone(), rela },
                        if rela {
                            (RawRela::size(layout.class) * table.relocations.len()) as _
                        } else {
                            (RawRel::size(layout.class) * table.relocations.len()) as _
                        },
                    )
                }
                ElfSectionContent::Group(group) => {
                    layout.add_part(
                        Part::Group(id.clone()),
                        RawGroupFlags::size(layout.class)
                            + u32::size(layout.class) * group.sections.len(),
                    );
                }
                ElfSectionContent::Hash(hash) => {
                    let size = <u64 as RawTypeAsPointerSize>::size(layout.class);
                    layout.add_part(
                        Part::Hash(id.clone()),
                        RawHashHeader::size(layout.class)
                            + hash.buckets.len() * size
                            + hash.chain.len(),
                    )
                }
                ElfSectionContent::Note(_) => {
                    return Err(WriteLayoutError::WritingNotesUnsupported);
                }
                ElfSectionContent::Unknown(_) => {
                    return Err(WriteLayoutError::UnknownSection);
                }
            }
        }

        // We need sections belonging to the same segment to reside adjacent in memory, otherwise
        // the segment can't load them as a block. To do so, we first add to the layout all of the
        // sections belonging to each segment, without page alignment between them, and only after
        // we add the sections that didn't belong to any segment.

        for segment in &object.segments {
            layout.align_to_page();
            let ElfSegmentContent::Sections(segment_sections) = &segment.content else {
                continue;
            };
            for section_id in segment_sections {
                let Some(deferred) = deferred_program_sections.remove(section_id) else {
                    continue;
                };
                layout.add_part(Part::ProgramSection(section_id.clone()), deferred.len);
            }
        }
        for (id, deferred) in deferred_program_sections {
            layout.align_to_page();
            layout.add_part(Part::ProgramSection(id), deferred.len);
        }
        layout.align_to_page();

        Ok(layout)
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
    Padding(PaddingId),
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub(super) struct PaddingId(usize);

#[derive(Debug)]
pub(super) struct PartMetadata {
    pub(super) len: u64,
    pub(super) offset: u64,
}

struct DeferredProgramSection {
    len: usize,
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
