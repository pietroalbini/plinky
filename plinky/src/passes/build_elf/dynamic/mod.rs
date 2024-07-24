mod sysv_hash;

use crate::passes::build_elf::dynamic::sysv_hash::create_sysv_hash;
use crate::passes::build_elf::relocations::create_rela;
use crate::passes::build_elf::symbols::create_symbols;
use crate::passes::build_elf::ElfBuilder;
use crate::passes::layout::SectionLayout;
use crate::repr::segments::{Segment, SegmentContent, SegmentType};
use crate::repr::symbols::views::DynamicSymbols;
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
            SectionLayout::Allocated { address } => address,
            SectionLayout::NotAllocated => panic!("section should be allocated"),
        }
    }};
}

pub(crate) fn add(builder: &mut ElfBuilder) {
    let bits = builder.object.env.class;
    let mut segment_sections = Vec::new();

    let string_table_id = builder.ids.allocate_section_id();
    let symbols = create_symbols(
        &builder.object.symbols,
        &DynamicSymbols,
        &mut builder.ids,
        &mut builder.sections,
        string_table_id,
        true,
    );

    let dynstr_len = symbols.string_table.content_size(bits);
    let dynstr_addr =
        add_section!(builder, segment_sections, ".dynstr", symbols.string_table, string_table_id);

    let dynsym = builder.ids.allocate_section_id();
    let dynsym_addr =
        add_section!(builder, segment_sections, ".dynsym", symbols.symbol_table, dynsym);

    let rela = create_rela(
        builder.object.dynamic_relocations.iter(),
        builder.object.env.class,
        builder.sections.zero_id,
        dynsym,
        &symbols.conversion,
    );
    let rela_len = rela.content_size(bits);
    let rela_addr = add_section!(builder, segment_sections, ".rela.dyn", rela);

    let hash_addr = add_section!(
        builder,
        segment_sections,
        ".hash",
        create_sysv_hash(
            builder.object.symbols.iter(&DynamicSymbols).map(|(_id, sym)| sym),
            dynsym,
        )
    );

    let dynamic_id = builder.ids.allocate_section_id();
    let dynamic_old_id = builder.old_ids.allocate_section_id();
    let dynamic = ElfSectionContent::Dynamic(ElfDynamic {
        string_table: string_table_id,
        directives: vec![
            ElfDynamicDirective::Hash { address: hash_addr.extract() },
            ElfDynamicDirective::StringTable { address: dynstr_addr.extract() },
            ElfDynamicDirective::StringTableSize { bytes: dynstr_len as _ },
            ElfDynamicDirective::SymbolTable { address: dynsym_addr.extract() },
            ElfDynamicDirective::SymbolTableEntrySize { bytes: RawSymbol::size(bits) as _ },
            ElfDynamicDirective::Rela { address: rela_addr.extract() },
            ElfDynamicDirective::RelaSize { bytes: rela_len as _ },
            ElfDynamicDirective::RelaEntrySize { bytes: RawRela::size(bits) as _ },
            ElfDynamicDirective::Flags1(ElfDynamicFlags1 { pie: true }),
            ElfDynamicDirective::Null,
        ],
    });
    add_section!(builder, segment_sections, ".dynamic", dynamic, dynamic_id, dynamic_old_id);

    builder.object.segments.push(Segment {
        align: 0x1000,
        type_: SegmentType::Program,
        perms: ElfPermissions::empty().read(),
        content: SegmentContent::Sections(segment_sections),
    });

    builder.object.segments.push(Segment {
        align: <u64 as RawTypeAsPointerSize>::size(bits) as _,
        type_: SegmentType::Dynamic,
        perms: ElfPermissions::empty().read(),
        content: SegmentContent::Sections(vec![dynamic_old_id]),
    });
    for type_ in [SegmentType::Program, SegmentType::ProgramHeader] {
        builder.object.segments.push(Segment {
            align: 0x1000,
            type_,
            perms: ElfPermissions::empty().read(),
            content: SegmentContent::ProgramHeader,
        });
    }
    builder.object.segments.push(Segment {
        align: 0x1000,
        type_: SegmentType::Program,
        perms: ElfPermissions::empty().read(),
        content: SegmentContent::ElfHeader,
    });
}
