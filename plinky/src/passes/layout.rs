use plinky_elf::ids::serial::SectionId;
use std::collections::BTreeMap;
use crate::repr::object::{DataSectionPart, Object, SectionContent};

const BASE_ADDRESS: u64 = 0x400000;
const PAGE_SIZE: u64 = 0x1000;

pub(crate) fn run(object: &Object) -> Layout {
    let mut layout = Layout { sections: BTreeMap::new() };
    let mut calculator = LayoutCalculator::new();
    for section in object.sections.values() {
        let mut section_calculator = calculator.begin_section();
        match &section.content {
            SectionContent::Data(parts) => {
                for (&id, part) in &parts.parts {
                    match part {
                        DataSectionPart::Real(real) => {
                            layout
                                .sections
                                .insert(id, section_calculator.layout_of(real.bytes.0.len() as _));
                        }
                        DataSectionPart::DeduplicationFacade(_) => {} // No layout for them.
                    }
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
}

impl Layout {
    pub(crate) fn of(&self, section: SectionId) -> u64 {
        self.sections.get(&section).expect("TODO").address
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
