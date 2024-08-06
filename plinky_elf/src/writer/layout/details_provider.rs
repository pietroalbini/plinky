use crate::ids::ElfIds;
use crate::writer::layout::Part;
use crate::writer::WriteLayoutError;
use crate::{ElfClass, ElfObject, ElfSectionContent};

pub trait LayoutDetailsProvider<I: ElfIds> {
    fn class(&self) -> ElfClass;

    fn sections_count(&self) -> usize;
    fn segments_count(&self) -> usize;

    fn program_section_len(&self, id: &I::SectionId) -> usize;
    fn string_table_len(&self, id: &I::SectionId) -> usize;
    fn symbols_in_table_count(&self, id: &I::SectionId) -> usize;
    fn sections_in_group_count(&self, id: &I::SectionId) -> usize;
    fn dynamic_directives_count(&self, id: &I::SectionId) -> usize;
    fn relocations_in_table_count(&self, id: &I::SectionId) -> usize;
    fn hash_details(&self, id: &I::SectionId) -> LayoutHashDetails;

    fn parts_for_sections(
        &self,
    ) -> Result<Vec<(I::SectionId, Part<I::SectionId>)>, WriteLayoutError>;

    fn object(&self) -> &ElfObject<I>;
}

pub struct LayoutHashDetails {
    pub buckets: usize,
    pub chain: usize,
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

impl<I: ElfIds> LayoutDetailsProvider<I> for ElfObject<I> {
    fn class(&self) -> ElfClass {
        self.env.class
    }

    fn sections_count(&self) -> usize {
        self.sections.len()
    }

    fn segments_count(&self) -> usize {
        self.segments.len()
    }

    fn program_section_len(&self, id: &I::SectionId) -> usize {
        cast_section!(self, id, Program).raw.len()
    }

    fn string_table_len(&self, id: &I::SectionId) -> usize {
        cast_section!(self, id, StringTable).len()
    }

    fn symbols_in_table_count(&self, id: &I::SectionId) -> usize {
        cast_section!(self, id, SymbolTable).symbols.len()
    }

    fn sections_in_group_count(&self, id: &I::SectionId) -> usize {
        cast_section!(self, id, Group).sections.len()
    }

    fn dynamic_directives_count(&self, id: &I::SectionId) -> usize {
        cast_section!(self, id, Dynamic).directives.len()
    }

    fn relocations_in_table_count(&self, id: &I::SectionId) -> usize {
        cast_section!(self, id, RelocationsTable).relocations.len()
    }

    fn hash_details(&self, id: &I::SectionId) -> LayoutHashDetails {
        let hash = cast_section!(self, id, Hash);
        LayoutHashDetails { buckets: hash.buckets.len(), chain: hash.chain.len() }
    }

    fn parts_for_sections(
        &self,
    ) -> Result<Vec<(I::SectionId, Part<I::SectionId>)>, WriteLayoutError> {
        let mut result = Vec::new();
        for (id, section) in &self.sections {
            let part = match &section.content {
                ElfSectionContent::Null => continue,
                ElfSectionContent::Program(_) => {
                    Part::ProgramSection(id.clone())
                }
                ElfSectionContent::Uninitialized(_) => {
                    // Uninitialized sections are not part of the file layout.
                    continue;
                }
                ElfSectionContent::SymbolTable(_) => {
                    Part::SymbolTable(id.clone())
                }
                ElfSectionContent::StringTable(_) => {
                    Part::StringTable(id.clone())
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
                        Part::Rela(id.clone())
                    } else {
                        Part::Rel(id.clone())
                    }
                }
                ElfSectionContent::Group(_) => Part::Group(id.clone()),
                ElfSectionContent::Hash(_) => Part::Hash(id.clone()),
                ElfSectionContent::Dynamic(_) => Part::Dynamic(id.clone()),

                ElfSectionContent::Note(_) => {
                    return Err(WriteLayoutError::WritingNotesUnsupported);
                }
                ElfSectionContent::Unknown(_) => {
                    return Err(WriteLayoutError::UnknownSection);
                }
            };
            result.push((id.clone(), part));
        }
        Ok(result)
    }

    fn object(&self) -> &ElfObject<I> {
        self
    }
}
