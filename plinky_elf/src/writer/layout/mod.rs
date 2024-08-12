mod details_provider;
mod part;

pub use self::details_provider::*;
pub use self::part::*;

use crate::ids::ElfIds;
use crate::raw::{
    RawGroupFlags, RawHashHeader, RawHeader, RawIdentification, RawProgramHeader, RawRel, RawRela,
    RawSectionHeader, RawSymbol,
};
use plinky_macros::{Display, Error};
use plinky_utils::ints::Address;
use plinky_utils::ints::ExtractNumber;
use plinky_utils::ints::Length;
use plinky_utils::ints::Offset;
use plinky_utils::ints::OutOfBoundsError;
use plinky_utils::raw_types::{RawType, RawTypeAsPointerSize};
use std::collections::BTreeMap;

#[derive(Debug)]
pub struct Layout<I: ElfIds> {
    parts: Vec<Part<I::SectionId>>,
    metadata: BTreeMap<Part<I::SectionId>, PartMetadata>,
}

impl<I: ElfIds> Layout<I> {
    pub fn new(
        details: &dyn LayoutDetailsProvider<I>,
        base_memory_address: Option<Address>,
    ) -> Result<Self, LayoutError> {
        let builder = LayoutBuilder {
            details,
            layout: Layout { parts: Vec::new(), metadata: BTreeMap::new() },
            current_offset: 0.into(),
            current_memory_address: base_memory_address,
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
            .filter(|(key, _)| key.section_id() == Some(id))
            .map(|(_, value)| value)
            .next()
            .unwrap()
    }

    pub fn convert_ids<T: ElfIds>(self, map: &BTreeMap<I::SectionId, T::SectionId>) -> Layout<T> {
        Layout {
            parts: self.parts.into_iter().map(|p| p.convert_ids(map)).collect(),
            metadata: self.metadata.into_iter().map(|(k, v)| (k.convert_ids(map), v)).collect(),
        }
    }
}

struct LayoutBuilder<'a, I: ElfIds> {
    details: &'a dyn LayoutDetailsProvider<I>,
    layout: Layout<I>,
    current_offset: Offset,
    current_memory_address: Option<Address>,
}

impl<I: ElfIds> LayoutBuilder<'_, I> {
    fn build(mut self) -> Result<Layout<I>, LayoutError> {
        self.add_part(Part::Header)?;
        self.add_part(Part::ProgramHeaders)?;
        self.add_part(Part::SectionHeaders)?;

        let groups = self.details.parts_groups()?;
        let part_to_group = groups
            .iter()
            .enumerate()
            .flat_map(|(idx, group)| group.parts.iter().map(move |part| (part, idx)))
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
        let mut put_in_groups = BTreeMap::new();
        for part in self.details.parts_for_sections()? {
            if let Some(group) = part_to_group.get(&part) {
                put_in_groups.entry(*group).or_insert_with(Vec::new).push(part);
            } else {
                put_in_preamble.push(part);
            }
        }
        for part in put_in_preamble {
            self.add_part(part)?;
        }
        for (group_idx, parts) in put_in_groups {
            self.align(groups[group_idx].align)?;
            for part in parts {
                self.add_part_in_memory(part)?;
            }
        }

        Ok(self.layout)
    }

    fn add_part(&mut self, part: Part<I::SectionId>) -> Result<(), LayoutError> {
        self.add_part_inner(part, false)
    }

    fn add_part_in_memory(&mut self, part: Part<I::SectionId>) -> Result<(), LayoutError> {
        self.add_part_inner(part, true)
    }

    fn add_part_inner(
        &mut self,
        part: Part<I::SectionId>,
        add_in_memory: bool,
    ) -> Result<(), LayoutError> {
        let len = part_len(self.details, &part);
        self.layout.parts.push(part.clone());

        let memory = if add_in_memory {
            match self.current_memory_address {
                Some(address) => {
                    self.current_memory_address = Some(address.offset(len.as_offset()?)?);
                    Some(PartMemory { len, address })
                }
                None => None,
            }
        } else {
            None
        };

        if part.present_in_file() {
            self.layout.metadata.insert(
                part,
                PartMetadata { file: Some(PartFile { len, offset: self.current_offset }), memory },
            );
            self.current_offset = self.current_offset.add(len.as_offset()?)?;
        } else {
            self.layout.metadata.insert(part, PartMetadata { file: None, memory });
        }

        Ok(())
    }

    fn align(&mut self, align: u64) -> Result<(), LayoutError> {
        // Align memory address.
        match &mut self.current_memory_address {
            Some(address) => *address = address.align(align)?,
            None => {}
        }

        // Align file offset.
        let len = self.current_offset.extract() as u64;
        if len % align == 0 {
            return Ok(());
        }
        let bytes_to_pad = align - len % align;
        self.add_part(Part::Padding { id: PaddingId::next(), len: bytes_to_pad as _ })?;

        Ok(())
    }
}

fn part_len<I: ElfIds>(
    details: &dyn LayoutDetailsProvider<I>,
    part: &Part<I::SectionId>,
) -> Length {
    let class = details.class();
    match part {
        Part::Header => RawIdentification::size(class) + RawHeader::size(class),

        Part::SectionHeaders => RawSectionHeader::size(class) * details.sections_count(),
        Part::ProgramHeaders => RawProgramHeader::size(class) * details.segments_count(),

        Part::ProgramSection(id) => details.program_section_len(id),
        Part::UninitializedSection(id) => details.uninitialized_section_len(id),
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
    .into()
}

#[derive(Debug, Error, Display)]
pub enum LayoutError {
    #[display("relocation section mixing rel and rela")]
    MixedRelRela,
    #[display("writing notes is not supported yet")]
    WritingNotesUnsupported,
    #[display("unkown section encountered while calculating the layout")]
    UnknownSection,
    #[display("the linker output is too large")]
    OutOFBounds(#[from] OutOfBoundsError),
}
