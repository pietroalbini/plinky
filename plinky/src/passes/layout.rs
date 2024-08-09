use crate::cli::Mode;
use crate::passes::build_elf::sysv_hash::num_buckets;
use crate::repr::object::Object;
use crate::repr::sections::SectionContent;
use crate::repr::segments::{SegmentContent, SegmentType};
use crate::utils::ints::Address;
use plinky_elf::ids::serial::SectionId;
use plinky_elf::raw::{RawHashHeader, RawRela, RawSymbol};
use plinky_utils::raw_types::{RawType, RawTypeAsPointerSize};
use std::collections::{BTreeMap, BTreeSet};

const PAGE_SIZE: u64 = 0x1000;
const STATIC_BASE_ADDRESS: u64 = 0x400000;
const PIE_BASE_ADDRESS: u64 = PAGE_SIZE;

pub(crate) fn run(object: &mut Object) -> Layout {
    let mut layout = Layout {
        current_address: match object.mode {
            Mode::PositionDependent => STATIC_BASE_ADDRESS,
            Mode::PositionIndependent => PIE_BASE_ADDRESS,
        },
        sections: BTreeMap::new(),
    };

    let mut not_allocated = object.sections.iter().map(|s| s.id).collect::<BTreeSet<_>>();
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

            not_allocated.remove(&id);
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

pub(crate) struct Layout {
    current_address: u64,
    sections: BTreeMap<SectionId, SectionLayout>,
}

impl Layout {
    pub(crate) fn of_section(&self, id: SectionId) -> &SectionLayout {
        match self.sections.get(&id) {
            Some(layout) => layout,
            None => panic!("section {id:?} doesn't have a layout"),
        }
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
