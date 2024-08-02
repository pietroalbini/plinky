use crate::cli::Mode;
use crate::passes::build_elf::sysv_hash::num_buckets;
use crate::passes::deduplicate::Deduplication;
use crate::repr::object::Object;
use crate::repr::sections::SectionContent;
use crate::repr::segments::{Segment, SegmentContent, SegmentType};
use crate::utils::ints::{Address, Offset, OutOfBoundsError};
use plinky_elf::ids::serial::SectionId;
use plinky_elf::raw::{RawHashHeader, RawRela, RawSymbol};
use plinky_macros::{Display, Error};
use plinky_utils::raw_types::{RawType, RawTypeAsPointerSize};
use std::collections::{BTreeMap, BTreeSet};

const PAGE_SIZE: u64 = 0x1000;
const STATIC_BASE_ADDRESS: u64 = 0x400000;
const PIE_BASE_ADDRESS: u64 = PAGE_SIZE;

pub(crate) fn run(
    object: &mut Object,
    deduplications: BTreeMap<SectionId, Deduplication>,
) -> Layout {
    let mut not_allocated = Vec::new();
    create_segments(object, &mut not_allocated);

    let mut layout = Layout {
        current_address: match object.mode {
            Mode::PositionDependent => STATIC_BASE_ADDRESS,
            Mode::PositionIndependent => PIE_BASE_ADDRESS,
        },
        sections: BTreeMap::new(),
        deduplications,
    };
    for segment in object.segments.iter() {
        let SegmentContent::Sections(sections) = &segment.content else { continue };
        match segment.type_ {
            // Need to be allocated:
            SegmentType::ProgramHeader => {}
            SegmentType::Interpreter => {}
            SegmentType::Program => {}
            SegmentType::Uninitialized => {}
            // Should already be allocated separately:
            SegmentType::Dynamic => continue,
        }
        for id in sections {
            if layout.sections.contains_key(id) {
                panic!("trying to layout the same section twice");
            }
            let len = section_len(&object, *id);
            layout.add_section(*id, len);
        }
        layout.page_align();
    }

    for id in not_allocated {
        layout.sections.insert(id, SectionLayout::NotAllocated);
    }

    layout
}

fn section_len(object: &Object, id: SectionId) -> u64 {
    match &object.sections.get(id).unwrap().content {
        SectionContent::Data(data) => data.bytes.len() as u64,
        SectionContent::Uninitialized(uninit) => uninit.len,

        SectionContent::StringsForSymbols(strings) => object
            .symbols
            .iter(&*strings.view)
            .map(|(_, symbol)| symbol.name().resolve().len() + 1 /* null byte */)
            .chain(std::iter::once(1)) // Null symbol
            .sum::<usize>() as u64,

        SectionContent::Symbols(symbols) => {
            (object.symbols.iter(&*symbols.view).count() * RawSymbol::size(object.env.class)) as u64
        }

        SectionContent::SysvHash(sysv) => {
            let symbols_count = object.symbols.iter(&*sysv.view).count();
            let buckets_len = num_buckets(symbols_count) * u32::size(object.env.class);
            let chain_len = symbols_count * u32::size(object.env.class);
            (RawHashHeader::size(object.env.class) + buckets_len + chain_len) as u64
        }

        SectionContent::Relocations(relocations) => {
            (RawRela::size(object.env.class) * relocations.relocations().len()) as u64
        }

        SectionContent::Dynamic(_) => {
            let entry_size = <u64 as RawTypeAsPointerSize>::size(object.env.class) as u64;

            // Increase by 1 to account for the implied null directive.
            let directives_count =
                object.dynamic_entries.iter().map(|d| d.directives_count() as u64).sum::<u64>() + 1;

            entry_size * directives_count
        }
    }
}

fn create_segments(object: &mut Object, not_allocated: &mut Vec<SectionId>) {
    // Segments can be created before the layout is generated. Ensure we don't put the sections in
    // them in two different segments.
    let sections_already_in_segments = object
        .segments
        .iter()
        .filter_map(|segment| match &segment.content {
            SegmentContent::ProgramHeader => None,
            SegmentContent::ElfHeader => None,
            SegmentContent::Sections(sections) => Some(sections),
        })
        .flatten()
        .collect::<BTreeSet<_>>();

    let mut segments = BTreeMap::new();
    for section in object.sections.iter() {
        if sections_already_in_segments.contains(&section.id) {
            continue;
        }
        let (type_, perms) = match &section.content {
            SectionContent::Data(data) => (SegmentType::Program, data.perms),
            SectionContent::Uninitialized(uninit) => (SegmentType::Uninitialized, uninit.perms),

            SectionContent::StringsForSymbols(_)
            | SectionContent::Symbols(_)
            | SectionContent::SysvHash(_)
            | SectionContent::Relocations(_)
            | SectionContent::Dynamic(_) => {
                not_allocated.push(section.id);
                continue;
            }
        };
        if perms.read || perms.write || perms.execute {
            segments.entry((type_, perms)).or_insert_with(Vec::new).push(section.id);
        } else {
            not_allocated.push(section.id);
        }
    }

    for ((type_, perms), sections) in segments {
        object.segments.add(Segment {
            align: PAGE_SIZE,
            type_,
            perms,
            content: SegmentContent::Sections(sections),
        });
    }
}

pub(crate) struct Layout {
    current_address: u64,
    sections: BTreeMap<SectionId, SectionLayout>,
    deduplications: BTreeMap<SectionId, Deduplication>,
}

impl Layout {
    pub(crate) fn of_section(&self, id: SectionId) -> &SectionLayout {
        match self.sections.get(&id) {
            Some(layout) => layout,
            None => panic!("section {id:?} doesn't have a layout"),
        }
    }

    pub(crate) fn address(
        &self,
        section: SectionId,
        offset: Offset,
    ) -> Result<(SectionId, Address), AddressResolutionError> {
        if let Some(deduplication) = self.deduplications.get(&section) {
            let base = match self.of_section(deduplication.target) {
                SectionLayout::Allocated { address, .. } => *address,
                SectionLayout::NotAllocated => {
                    return Err(AddressResolutionError::PointsToUnallocatedSection(
                        deduplication.target,
                    ))
                }
            };

            match deduplication.map.get(&offset) {
                Some(&mapped) => Ok((deduplication.target, base.offset(mapped)?)),
                None => Err(AddressResolutionError::UnalignedReferenceToDeduplication),
            }
        } else {
            match self.of_section(section) {
                SectionLayout::Allocated { address, .. } => Ok((section, address.offset(offset)?)),
                SectionLayout::NotAllocated => {
                    Err(AddressResolutionError::PointsToUnallocatedSection(section))
                }
            }
        }
    }

    pub(crate) fn iter_deduplications(&self) -> impl Iterator<Item = (SectionId, &Deduplication)> {
        self.deduplications.iter().map(|(id, dedup)| (*id, dedup))
    }

    pub(crate) fn add_section(&mut self, id: SectionId, len: u64) -> SectionLayout {
        let layout = SectionLayout::Allocated { address: self.current_address.into(), len };

        self.sections.insert(id, layout);
        self.current_address += len;

        layout
    }

    pub(crate) fn page_align(&mut self) {
        self.current_address = (self.current_address + PAGE_SIZE) & !(PAGE_SIZE - 1);
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub(crate) enum SectionLayout {
    Allocated { address: Address, len: u64 },
    NotAllocated,
}

#[derive(Debug, Display, Error)]
pub(crate) enum AddressResolutionError {
    #[display("address points to section {f0:?}, which is not going to be allocated in memory")]
    PointsToUnallocatedSection(SectionId),
    #[display("referenced an offset not aligned to the deduplication boundaries")]
    UnalignedReferenceToDeduplication,
    #[transparent]
    OutOfBounds(OutOfBoundsError),
}
