use crate::repr::object::{
    DataSection, DataSectionPart, Object, Section, SectionContent, SectionLayout,
    UninitializedSectionPart,
};

const BASE_ADDRESS: u64 = 0x400000;
const PAGE_SIZE: u64 = 0x1000;

pub(crate) fn run(object: Object<()>) -> Object<SectionLayout> {
    let mut calculator = LayoutCalculator::new();
    Object {
        env: object.env,
        sections: object
            .sections
            .into_iter()
            .map(|(name, section)| {
                let mut calculator = calculator.begin_section();
                (
                    name,
                    Section {
                        perms: section.perms,
                        content: match section.content {
                            SectionContent::Data(data) => SectionContent::Data(DataSection {
                                deduplication: data.deduplication,
                                parts: data
                                    .parts
                                    .into_iter()
                                    .map(|(id, part)| {
                                        (
                                            id,
                                            DataSectionPart {
                                                layout: calculator.layout_of(part.bytes.len() as _),
                                                bytes: part.bytes,
                                                relocations: part.relocations,
                                            },
                                        )
                                    })
                                    .collect(),
                            }),
                            SectionContent::Uninitialized(uninit) => SectionContent::Uninitialized(
                                uninit
                                    .into_iter()
                                    .map(|(id, part)| {
                                        (
                                            id,
                                            UninitializedSectionPart {
                                                layout: calculator.layout_of(part.len),
                                                len: part.len,
                                            },
                                        )
                                    })
                                    .collect(),
                            ),
                        },
                    },
                )
            })
            .collect(),
        section_ids_to_names: object.section_ids_to_names,
        strings: object.strings,
        symbols: object.symbols,
    }
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
