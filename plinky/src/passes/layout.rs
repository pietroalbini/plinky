use crate::passes::deduplicate::Deduplication;
use crate::repr::object::{Object, SectionContent};
use plinky_elf::ids::serial::SectionId;
use plinky_macros::{Display, Error};
use std::collections::BTreeMap;

const BASE_ADDRESS: u64 = 0x400000;
const PAGE_SIZE: u64 = 0x1000;

pub(crate) fn run(object: &Object, deduplications: BTreeMap<SectionId, Deduplication>) -> Layout {
    let mut layout = Layout { sections: BTreeMap::new(), deduplications };
    let mut calculator = LayoutCalculator::new();
    for section in object.sections.values() {
        let mut section_calculator = calculator.begin_section();
        match &section.content {
            SectionContent::Data(parts) => {
                for (&id, part) in &parts.parts {
                    layout
                        .sections
                        .insert(id, section_calculator.layout_of(part.bytes.0.len() as _));
                }
            }
            SectionContent::Uninitialized(parts) => {
                for (&id, part) in parts {
                    layout.sections.insert(id, section_calculator.layout_of(part.len));
                }
            }
        }
    }

    layout
}

pub(crate) struct Layout {
    sections: BTreeMap<SectionId, SectionLayout>,
    deduplications: BTreeMap<SectionId, Deduplication>,
}

impl Layout {
    pub(crate) fn of(&self, section: SectionId) -> u64 {
        match self.sections.get(&section) {
            Some(layout) => layout.address,
            None => panic!("section {section:?} doesn't have a layout"),
        }
    }

    pub(crate) fn address(
        &self,
        section: SectionId,
        offset: i64,
    ) -> Result<u64, AddressResolutionError> {
        if let Some(deduplication) = self.deduplications.get(&section) {
            let base = self
                .sections
                .get(&deduplication.target)
                .expect("deduplication doesn't point to a section with a layout");

            let map_key = u64::try_from(offset)
                .map_err(|_| AddressResolutionError::NegativeOffsetToAccessDeduplications)?;
            match deduplication.map.get(&map_key) {
                Some(&mapped) => Ok(base.address + mapped),
                None => Err(AddressResolutionError::UnalignedReferenceToDeduplication),
            }
        } else if let Some(layout) = self.sections.get(&section) {
            Ok((layout.address as i64 + offset) as u64)
        } else {
            panic!("section {section:?} doesn't have a layout");
        }
    }

    pub(crate) fn iter_deduplications(&self) -> impl Iterator<Item = (SectionId, &Deduplication)> {
        self.deduplications.iter().map(|(id, dedup)| (*id, dedup))
    }
}

struct SectionLayout {
    address: u64,
}

struct LayoutCalculator {
    address: u64,
}

impl LayoutCalculator {
    fn new() -> Self {
        Self { address: BASE_ADDRESS }
    }

    fn begin_section(&mut self) -> SectionLayoutCalculator<'_> {
        SectionLayoutCalculator { parent: self }
    }
}

struct SectionLayoutCalculator<'a> {
    parent: &'a mut LayoutCalculator,
}

impl SectionLayoutCalculator<'_> {
    fn layout_of(&mut self, len: u64) -> SectionLayout {
        let layout = SectionLayout { address: self.parent.address };
        self.parent.address += len;
        layout
    }
}

impl Drop for SectionLayoutCalculator<'_> {
    fn drop(&mut self) {
        // Align to the next page boundary when a section ends.
        self.parent.address = (self.parent.address + PAGE_SIZE) & !(PAGE_SIZE - 1);
    }
}

#[derive(Debug, Display, Error)]
pub(crate) enum AddressResolutionError {
    #[display("negative offset was used to access deduplications")]
    NegativeOffsetToAccessDeduplications,
    #[display("referenced an offset not aligned to the deduplication boundaries")]
    UnalignedReferenceToDeduplication,
}
