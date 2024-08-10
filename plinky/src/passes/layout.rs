use crate::cli::Mode;
use crate::passes::build_elf::sysv_hash::num_buckets;
use crate::repr::object::Object;
use crate::repr::sections::SectionContent;
use crate::repr::segments::{SegmentContent, SegmentType};
use plinky_utils::ints::Address;
use plinky_elf::ids::serial::{SectionId, SerialIds};
use plinky_elf::writer::layout::{
    Layout as ElfLayout, LayoutDetailsHash, LayoutDetailsProvider, LayoutDetailsSegment,
    LayoutError, Part, PartMetadata,
};
use plinky_elf::ElfClass;
use std::collections::BTreeMap;

pub(crate) fn run(object: &Object) -> Result<Layout, LayoutError> {
    let base_address = match object.mode {
        Mode::PositionDependent => 0x400000,
        Mode::PositionIndependent => 0x1000,
    };

    let elf_layout = ElfLayout::new(object, Some(base_address))?;

    let mut repr = BTreeMap::new();
    for part in elf_layout.parts() {
        let Some(id) = part.section_id() else { continue };
        let metadata = elf_layout.metadata(part);
        match &metadata.memory {
            Some(memory) => {
                repr.insert(
                    *id,
                    SectionLayout::Allocated { address: memory.address.into(), len: memory.len },
                );
            }
            None => {
                repr.insert(*id, SectionLayout::NotAllocated);
            }
        };
    }
    Ok(Layout { repr, elf: elf_layout })
}

macro_rules! cast_section {
    ($self:expr, $id:expr, $variant:ident) => {
        match $self.sections.get(*$id).map(|s| &s.content) {
            Some(SectionContent::$variant(inner)) => inner,
            Some(_) => panic!("section {:?} is of the wrong type", $id),
            None => panic!("section {:?} is missing", $id),
        }
    };
}

impl LayoutDetailsProvider<SerialIds> for Object {
    fn class(&self) -> ElfClass {
        self.env.class
    }

    fn sections_count(&self) -> usize {
        self.sections.len()
    }

    fn segments_count(&self) -> usize {
        self.segments.len()
    }

    fn program_section_len(&self, id: &SectionId) -> usize {
        cast_section!(self, id, Data).bytes.len()
    }

    fn uninitialized_section_len(&self, id: &SectionId) -> usize {
        cast_section!(self, id, Uninitialized).len as _
    }

    fn string_table_len(&self, id: &SectionId) -> usize {
        let strings = cast_section!(self, id, StringsForSymbols);
        self.symbols
            .iter(&*strings.view)
            .map(|(_, symbol)| symbol.name().resolve().len() + 1 /* null byte */)
            .chain(std::iter::once(1)) // Null symbol
            .sum::<usize>()
    }

    fn symbols_in_table_count(&self, id: &SectionId) -> usize {
        let symbols = cast_section!(self, id, Symbols);
        self.symbols.iter(&*symbols.view).count()
    }

    fn sections_in_group_count(&self, _id: &SectionId) -> usize {
        unimplemented!();
    }

    fn dynamic_directives_count(&self, _id: &SectionId) -> usize {
        self.dynamic_entries.iter().map(|d| d.directives_count()).sum::<usize>() + 1
    }

    fn relocations_in_table_count(&self, id: &SectionId) -> usize {
        cast_section!(self, id, Relocations).relocations().len()
    }

    fn hash_details(&self, id: &SectionId) -> LayoutDetailsHash {
        let hash = cast_section!(self, id, SysvHash);
        let symbols_count = self.symbols.iter(&*hash.view).count();
        LayoutDetailsHash { buckets: num_buckets(symbols_count), chain: symbols_count }
    }

    fn parts_for_sections(&self) -> Result<Vec<(SectionId, Part<SectionId>)>, LayoutError> {
        let mut result = Vec::new();
        for section in self.sections.iter() {
            result.push((
                section.id,
                match &section.content {
                    SectionContent::Data(_) => Part::ProgramSection(section.id),
                    SectionContent::Uninitialized(_) => Part::UninitializedSection(section.id),
                    SectionContent::StringsForSymbols(_) => Part::StringTable(section.id),
                    SectionContent::Symbols(_) => Part::SymbolTable(section.id),
                    SectionContent::SysvHash(_) => Part::Hash(section.id),
                    SectionContent::Relocations(_) => Part::Rela(section.id),
                    SectionContent::Dynamic(_) => Part::Dynamic(section.id),
                },
            ));
        }
        Ok(result)
    }

    fn loadable_segments(&self) -> Vec<LayoutDetailsSegment<SerialIds>> {
        let mut result = Vec::new();
        for segment in self.segments.iter() {
            match &segment.type_ {
                SegmentType::ProgramHeader => continue,
                SegmentType::Interpreter => {}
                SegmentType::Program => {}
                SegmentType::Uninitialized => {}
                SegmentType::Dynamic => continue,
            }
            match &segment.content {
                SegmentContent::ProgramHeader => continue,
                SegmentContent::ElfHeader => continue,
                SegmentContent::Sections(sections) => {
                    result.push(LayoutDetailsSegment { sections: sections.clone() });
                }
            }
        }
        result
    }
}

pub(crate) struct Layout {
    repr: BTreeMap<SectionId, SectionLayout>,
    elf: ElfLayout<SerialIds>,
}

impl Layout {
    pub(crate) fn parts(&self) -> &[Part<SectionId>] {
        self.elf.parts()
    }

    pub(crate) fn metadata(&self, part: &Part<SectionId>) -> &PartMetadata {
        self.elf.metadata(part)
    }

    pub(crate) fn of_section(&self, id: SectionId) -> &SectionLayout {
        self.repr.get(&id).unwrap()
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub(crate) enum SectionLayout {
    Allocated { address: Address, len: u64 },
    NotAllocated,
}
