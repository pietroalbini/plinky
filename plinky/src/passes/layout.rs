use crate::cli::Mode;
use crate::interner::Interned;
use crate::passes::build_elf::sysv_hash::num_buckets;
use crate::repr::object::Object;
use crate::repr::sections::{Section, SectionContent};
use crate::repr::segments::{SegmentContent, SegmentType};
use crate::repr::symbols::SymbolVisibility;
use plinky_elf::ids::serial::{SectionId, SerialIds};
use plinky_elf::writer::layout::{
    Layout, LayoutDetailsHash, LayoutDetailsProvider, LayoutError, LayoutPartsGroup, Part,
};
use plinky_elf::ElfClass;
use plinky_utils::ints::{Address, ExtractNumber};
use std::collections::BTreeSet;

pub(crate) fn run(object: &Object) -> Result<Layout<SerialIds>, LayoutError> {
    let base_address: Address = match object.mode {
        Mode::PositionDependent => 0x400000u64.into(),
        Mode::PositionIndependent => 0u64.into(),
        Mode::SharedLibrary => 0u64.into(),
    };

    Layout::new(object, Some(base_address))
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
        self.sections.len() + 1 /* null section */
    }

    fn segments_count(&self) -> usize {
        self.segments.len()
    }

    fn program_section_len(&self, id: &SectionId) -> usize {
        cast_section!(self, id, Data).bytes.len()
    }

    fn uninitialized_section_len(&self, id: &SectionId) -> usize {
        cast_section!(self, id, Uninitialized).len.extract() as _
    }

    fn string_table_len(&self, id: &SectionId) -> usize {
        let strings: Box<dyn Iterator<Item = Interned<String>>> =
            match self.sections.get(*id).map(|s| &s.content) {
                Some(SectionContent::StringsForSymbols(symbols)) => {
                    // Local symbols also have a STT_FILE entry for every file.
                    let file_names = self
                        .symbols
                        .iter(&*symbols.view)
                        .filter(|(_, s)| matches!(s.visibility(), SymbolVisibility::Local))
                        .filter_map(|(_, s)| s.stt_file())
                        .collect::<BTreeSet<_>>();

                    Box::new(
                        self.symbols
                            .iter(&*symbols.view)
                            .map(|(_, s)| s.name())
                            .chain(file_names.into_iter()),
                    )
                }
                Some(SectionContent::SectionNames) => {
                    Box::new(self.sections.iter().map(|s| s.name))
                }
                Some(_) => panic!("section {id:?} is of the wrong type"),
                None => panic!("section {id:?} is missing"),
            };

        strings
            .map(|s| s.resolve().len() + 1 /* null byte */)
            .chain(std::iter::once(1)) // Null symbol
            .sum::<usize>()
    }

    fn symbols_in_table_count(&self, id: &SectionId) -> usize {
        let symbols = cast_section!(self, id, Symbols);
        let symbols_count = self.symbols.iter(&*symbols.view).count();

        // Local symbols also have a STT_FILE entry for every file.
        let files_count = self
            .symbols
            .iter(&*symbols.view)
            .filter(|(_, s)| matches!(s.visibility(), SymbolVisibility::Local))
            .filter_map(|(_, s)| s.stt_file())
            .collect::<BTreeSet<_>>()
            .len();

        symbols_count + files_count
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

    fn parts_for_sections(&self) -> Result<Vec<Part<SectionId>>, LayoutError> {
        let mut result = Vec::new();

        result.push(Part::Header);
        result.push(Part::ProgramHeaders);
        result.push(Part::SectionHeaders);

        for section in self.sections.iter() {
            result.push(part_for_section(section));
        }
        Ok(result)
    }

    fn parts_groups(&self) -> Result<Vec<LayoutPartsGroup<SerialIds>>, LayoutError> {
        let mut result = Vec::new();
        for (_id, segment) in self.segments.iter() {
            match &segment.type_ {
                SegmentType::ProgramHeader => continue,
                SegmentType::Interpreter => continue,
                SegmentType::Program => {}
                SegmentType::Uninitialized => {}
                SegmentType::Dynamic => continue,
                SegmentType::GnuStack => continue,
                SegmentType::GnuRelro => continue,
            }

            let group = LayoutPartsGroup {
                align: segment.align,
                parts: segment
                    .content
                    .iter()
                    .map(|c| match c {
                        SegmentContent::ProgramHeader => Part::ProgramHeaders,
                        SegmentContent::ElfHeader => Part::Header,
                        SegmentContent::Section(id) => {
                            part_for_section(self.sections.get(*id).unwrap())
                        }
                    })
                    .collect(),
            };
            if !group.parts.is_empty() {
                result.push(group);
            }
        }
        Ok(result)
    }
}

fn part_for_section(section: &Section) -> Part<SectionId> {
    match &section.content {
        SectionContent::Data(_) => Part::ProgramSection(section.id),
        SectionContent::Uninitialized(_) => Part::UninitializedSection(section.id),
        SectionContent::StringsForSymbols(_) => Part::StringTable(section.id),
        SectionContent::Symbols(_) => Part::SymbolTable(section.id),
        SectionContent::SysvHash(_) => Part::Hash(section.id),
        SectionContent::Relocations(_) => Part::Rela(section.id),
        SectionContent::Dynamic(_) => Part::Dynamic(section.id),
        SectionContent::SectionNames => Part::StringTable(section.id),
    }
}
