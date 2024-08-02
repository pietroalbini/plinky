use crate::passes::build_elf::ElfBuilder;
use crate::passes::layout::SectionLayout;
use crate::repr::object::DynamicEntry;
use crate::repr::segments::{Segment, SegmentContent, SegmentType};
use crate::utils::ints::ExtractNumber;
use plinky_elf::raw::{RawRela, RawSymbol};
use plinky_elf::{
    ElfDynamic, ElfDynamicDirective, ElfDynamicFlags1, ElfPermissions, ElfSectionContent,
};
use plinky_utils::raw_types::{RawType, RawTypeAsPointerSize};

macro_rules! add_section {
    ($builder:expr, $segment_sections:expr, $name:expr, $content:expr) => {
        add_section!(
            $builder,
            $segment_sections,
            $name,
            $content,
            $builder.ids.allocate_section_id(),
            $builder.old_ids.allocate_section_id()
        )
    };
    ($builder:expr, $segment_sections:expr, $name:expr, $content:expr, $id:expr) => {
        add_section!(
            $builder,
            $segment_sections,
            $name,
            $content,
            $id,
            $builder.old_ids.allocate_section_id()
        )
    };
    ($builder:expr, $segment_sections:expr, $name:expr, $content:expr, $id:expr, $old_id:expr) => {{
        let content = $content;
        let len = content.content_size($builder.object.env.class);
        let old_id = $old_id;
        let layout = $builder.layout.add_section(old_id, len as _);
        $segment_sections.push(old_id);
        $builder.sections.create($name, content).layout(&layout).old_id(old_id).add_with_id($id);
        match layout {
            SectionLayout::Allocated { address, .. } => address,
            SectionLayout::NotAllocated => panic!("section should be allocated"),
        }
    }};
}

pub(super) fn add(builder: &mut ElfBuilder) {
    let bits = builder.object.env.class;
    let mut segment_sections = Vec::new();

    let dynstr = builder
        .object
        .dynamic_entries
        .iter()
        .filter_map(|entry| match entry {
            DynamicEntry::StringTable(id) => Some(*id),
            _ => None,
        })
        .next()
        .expect("dynstr not generated");
    let dynstr_new = builder.sections.new_id_of(dynstr);

    let mut directives = Vec::new();
    for entry in &builder.object.dynamic_entries {
        match entry {
            DynamicEntry::StringTable(id) => {
                let SectionLayout::Allocated { address, len } = builder.layout.of_section(*id)
                else {
                    panic!("non-allocated dynamic string table");
                };
                directives.push(ElfDynamicDirective::StringTable { address: address.extract() });
                directives.push(ElfDynamicDirective::StringTableSize { bytes: *len });
            }
            DynamicEntry::SymbolTable(id) => {
                let SectionLayout::Allocated { address, .. } = builder.layout.of_section(*id)
                else {
                    panic!("non-allocated dynamic symbol table");
                };
                directives.push(ElfDynamicDirective::SymbolTable { address: address.extract() });
                directives.push(ElfDynamicDirective::SymbolTableEntrySize {
                    bytes: RawSymbol::size(bits) as _,
                });
            }
            DynamicEntry::Hash(id) => {
                let SectionLayout::Allocated { address, .. } = builder.layout.of_section(*id)
                else {
                    panic!("non-allocated sysv hash table");
                };
                directives.push(ElfDynamicDirective::Hash { address: address.extract() });
            }
            DynamicEntry::Rela(id) => {
                let SectionLayout::Allocated { address, len } = builder.layout.of_section(*id)
                else {
                    panic!("non-allocated rela section");
                };
                directives.push(ElfDynamicDirective::Rela { address: address.extract() });
                directives.push(ElfDynamicDirective::RelaSize { bytes: *len });
                directives
                    .push(ElfDynamicDirective::RelaEntrySize { bytes: RawRela::size(bits) as _ });
            }
            DynamicEntry::PieFlag => {
                directives.push(ElfDynamicDirective::Flags1(ElfDynamicFlags1 { pie: true }));
            }
        }
    }

    directives.push(ElfDynamicDirective::Null);

    let dynamic_id = builder.ids.allocate_section_id();
    let dynamic_old_id = builder.old_ids.allocate_section_id();
    let dynamic = ElfSectionContent::Dynamic(ElfDynamic { string_table: dynstr_new, directives });
    add_section!(builder, segment_sections, ".dynamic", dynamic, dynamic_id, dynamic_old_id);

    builder.object.segments.add(Segment {
        align: 0x1000,
        type_: SegmentType::Program,
        perms: ElfPermissions::empty().read(),
        content: SegmentContent::Sections(segment_sections),
    });

    builder.object.segments.add(Segment {
        align: <u64 as RawTypeAsPointerSize>::size(bits) as _,
        type_: SegmentType::Dynamic,
        perms: ElfPermissions::empty().read(),
        content: SegmentContent::Sections(vec![dynamic_old_id]),
    });
    for type_ in [SegmentType::Program, SegmentType::ProgramHeader] {
        builder.object.segments.add(Segment {
            align: 0x1000,
            type_,
            perms: ElfPermissions::empty().read(),
            content: SegmentContent::ProgramHeader,
        });
    }
    builder.object.segments.add(Segment {
        align: 0x1000,
        type_: SegmentType::Program,
        perms: ElfPermissions::empty().read(),
        content: SegmentContent::ElfHeader,
    });
}
