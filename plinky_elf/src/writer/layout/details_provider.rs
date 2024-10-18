use crate::ids::ElfSectionId;
use crate::writer::layout::Part;
use crate::writer::LayoutError;
use crate::{ElfClass, ElfObject, ElfSection, ElfSectionContent, ElfSegmentType};

pub trait LayoutDetailsProvider<S: Copy> {
    fn class(&self) -> ElfClass;

    fn sections_count(&self) -> usize;
    fn segments_count(&self) -> usize;

    fn program_section_len(&self, id: S) -> usize;
    fn uninitialized_section_len(&self, id: S) -> usize;
    fn string_table_len(&self, id: S) -> usize;
    fn symbols_in_table_count(&self, id: S) -> usize;
    fn sections_in_group_count(&self, id: S) -> usize;
    fn dynamic_directives_count(&self, id: S) -> usize;
    fn relocations_in_table_count(&self, id: S) -> usize;
    fn hash_details(&self, id: S) -> LayoutDetailsHash;
    fn note_details(&self, id: S) -> Vec<LayoutDetailsNote>;

    fn parts_for_sections(&self) -> Result<Vec<Part<S>>, LayoutError>;
    fn parts_groups(&self) -> Result<Vec<LayoutPartsGroup<S>>, LayoutError>;
}

pub struct LayoutDetailsHash {
    pub buckets: usize,
    pub chain: usize,
}

pub struct LayoutDetailsNote {
    pub name_len: usize,
    pub value_len: usize,
}

pub struct LayoutPartsGroup<S> {
    pub align: u64,
    pub parts: Vec<Part<S>>,
}

macro_rules! cast_section {
    ($self:expr, $id:expr, $variant:ident) => {
        match $self.sections.get(&$id).map(|s| &s.content) {
            Some(ElfSectionContent::$variant(inner)) => inner,
            Some(_) => panic!("section {:?} is of the wrong type", $id),
            None => panic!("missing section {:?}", $id),
        }
    };
}

impl LayoutDetailsProvider<ElfSectionId> for ElfObject {
    fn class(&self) -> ElfClass {
        self.env.class
    }

    fn sections_count(&self) -> usize {
        self.sections.len()
    }

    fn segments_count(&self) -> usize {
        self.segments.len()
    }

    fn program_section_len(&self, id: ElfSectionId) -> usize {
        cast_section!(self, id, Program).raw.len()
    }

    fn uninitialized_section_len(&self, id: ElfSectionId) -> usize {
        cast_section!(self, id, Uninitialized).len as _
    }

    fn string_table_len(&self, id: ElfSectionId) -> usize {
        cast_section!(self, id, StringTable).len()
    }

    fn symbols_in_table_count(&self, id: ElfSectionId) -> usize {
        cast_section!(self, id, SymbolTable).symbols.len()
    }

    fn sections_in_group_count(&self, id: ElfSectionId) -> usize {
        cast_section!(self, id, Group).sections.len()
    }

    fn dynamic_directives_count(&self, id: ElfSectionId) -> usize {
        cast_section!(self, id, Dynamic).directives.len()
    }

    fn relocations_in_table_count(&self, id: ElfSectionId) -> usize {
        cast_section!(self, id, RelocationsTable).relocations.len()
    }

    fn hash_details(&self, id: ElfSectionId) -> LayoutDetailsHash {
        let hash = cast_section!(self, id, Hash);
        LayoutDetailsHash { buckets: hash.buckets.len(), chain: hash.chain.len() }
    }

    fn note_details(&self, id: ElfSectionId) -> Vec<LayoutDetailsNote> {
        cast_section!(self, id, Note)
            .notes
            .iter()
            .map(|note| LayoutDetailsNote {
                name_len: note.name().len(),
                value_len: note.value_len(self.env.class),
            })
            .collect()
    }

    fn parts_for_sections(&self) -> Result<Vec<Part<ElfSectionId>>, LayoutError> {
        let mut result = Vec::new();

        result.push(Part::Header);
        result.push(Part::ProgramHeaders);
        result.push(Part::SectionHeaders);

        for (id, section) in &self.sections {
            let Some(part) = part_for_section(*id, section)? else { continue };
            result.push(part);
        }
        Ok(result)
    }

    fn parts_groups(&self) -> Result<Vec<LayoutPartsGroup<ElfSectionId>>, LayoutError> {
        let mut groups = Vec::new();
        for segment in &self.segments {
            match &segment.type_ {
                ElfSegmentType::ProgramHeaderTable => continue,
                ElfSegmentType::Interpreter => {}
                ElfSegmentType::Load => {}
                ElfSegmentType::Dynamic => continue,
                ElfSegmentType::Note => continue,
                ElfSegmentType::GnuStack => continue,
                ElfSegmentType::GnuRelro => continue,
                ElfSegmentType::GnuProperty => continue,
                ElfSegmentType::Null => continue,
                ElfSegmentType::Unknown(_) => continue,
            };

            let mut group = LayoutPartsGroup { align: segment.align, parts: Vec::new() };
            let range = segment.virtual_address..=(segment.virtual_address + segment.memory_size);
            for (id, section) in &self.sections {
                if section.memory_address == 0 || !range.contains(&section.memory_address) {
                    continue;
                }
                let Some(part) = part_for_section(*id, section)? else { continue };
                group.parts.push(part);
            }
            if !group.parts.is_empty() {
                groups.push(group);
            }
        }
        Ok(groups)
    }
}

fn part_for_section(
    id: ElfSectionId,
    section: &ElfSection,
) -> Result<Option<Part<ElfSectionId>>, LayoutError> {
    Ok(Some(match &section.content {
        ElfSectionContent::Null => return Ok(None),
        ElfSectionContent::Program(_) => Part::ProgramSection(id.clone()),
        ElfSectionContent::Uninitialized(_) => Part::UninitializedSection(id.clone()),
        ElfSectionContent::SymbolTable(_) => Part::SymbolTable(id.clone()),
        ElfSectionContent::StringTable(_) => Part::StringTable(id.clone()),

        ElfSectionContent::RelocationsTable(table) => {
            let mut rela = None;
            for relocation in &table.relocations {
                match rela {
                    Some(rela) if rela == relocation.addend.is_some() => {}
                    Some(_) => return Err(LayoutError::MixedRelRela),
                    None => rela = Some(relocation.addend.is_some()),
                }
            }
            let rela = rela.unwrap_or(false);
            if rela {
                Part::Rela(id.clone())
            } else {
                Part::Rel(id.clone())
            }
        }
        ElfSectionContent::Group(_) => Part::Group(id.clone()),
        ElfSectionContent::Hash(_) => Part::Hash(id.clone()),
        ElfSectionContent::Dynamic(_) => Part::Dynamic(id.clone()),

        ElfSectionContent::Note(_) => Part::Note(id.clone()),
        ElfSectionContent::Unknown(_) => {
            return Err(LayoutError::UnknownSection);
        }
    }))
}
