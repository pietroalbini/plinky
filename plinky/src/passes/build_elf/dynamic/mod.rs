mod sysv_hash;

use super::symbols::SymbolTableKind;
use crate::passes::build_elf::dynamic::sysv_hash::create_sysv_hash;
use crate::passes::build_elf::relocations::create_rela;
use crate::passes::build_elf::symbols::create_symbols;
use crate::passes::build_elf::ElfBuilder;
use crate::passes::layout::{SectionLayout, SegmentType};
use crate::utils::ints::ExtractNumber;
use plinky_elf::raw::{RawRela, RawSymbol};
use plinky_elf::{ElfDynamic, ElfDynamicDirective, ElfPermissions, ElfSectionContent};
use plinky_utils::raw_types::RawType;

macro_rules! add_section {
    ($builder:expr, $segment:expr, $name:expr, $content:expr) => {
        add_section!($builder, $segment, $name, $content, $builder.ids.allocate_section_id())
    };
    ($builder:expr, $segment:expr, $name:expr, $content:expr, $id:expr) => {{
        let content = $content;
        let len = content.content_size($builder.object.env.class);
        let old_id = $builder.old_ids.allocate_section_id();
        let layout = $segment.add_section(old_id, len as _);
        $builder.sections.create($name, content).layout(&layout).old_id(old_id).add_with_id($id);
        match layout {
            SectionLayout::Allocated { address } => address,
            SectionLayout::NotAllocated => panic!("section should be allocated"),
        }
    }};
}

pub(crate) fn add(builder: &mut ElfBuilder) {
    let bits = builder.object.env.class;
    let mut segment = builder.layout.prepare_segment();

    let symbols = create_symbols(
        builder.object.symbols.iter_dynamic_symbols(),
        builder.object.symbols.null_symbol_id(),
        &mut builder.ids,
        &mut builder.sections,
        SymbolTableKind::DynSym,
    );

    let dynstr_len = symbols.string_table.content_size(bits);
    let dynstr_addr =
        add_section!(builder, segment, ".dynstr", symbols.string_table, symbols.string_table_id);

    let dynsym = builder.ids.allocate_section_id();
    let dynsym_addr = add_section!(builder, segment, ".dynsym", symbols.symbol_table, dynsym);

    let rela = create_rela(
        builder.object.dynamic_relocations.iter(),
        builder.object.env.class,
        builder.sections.zero_id,
        dynsym,
        &symbols.conversion,
    );
    let rela_len = rela.content_size(bits);
    let rela_addr = add_section!(builder, segment, ".rela.dyn", rela);

    let hash_addr = add_section!(
        builder,
        segment,
        ".hash",
        create_sysv_hash(
            builder.object.symbols.iter_dynamic_symbols().map(|(_id, sym)| sym),
            dynsym,
        )
    );

    let dynamic = vec![
        ElfDynamicDirective::Hash { address: hash_addr.extract() },
        ElfDynamicDirective::StringTable { address: dynstr_addr.extract() },
        ElfDynamicDirective::StringTableSize { bytes: dynstr_len as _ },
        ElfDynamicDirective::SymbolTable { address: dynsym_addr.extract() },
        ElfDynamicDirective::SymbolTableEntrySize { bytes: RawSymbol::size(bits) as _ },
        ElfDynamicDirective::Rela { address: rela_addr.extract() },
        ElfDynamicDirective::RelaSize { bytes: rela_len as _ },
        ElfDynamicDirective::RelaEntrySize { bytes: RawRela::size(bits) as _ },
        ElfDynamicDirective::Null,
    ];

    add_section!(
        builder,
        segment,
        ".dynamic",
        ElfSectionContent::Dynamic(ElfDynamic { directives: dynamic })
    );

    segment.finalize(
        SegmentType::Program,
        ElfPermissions { read: true, write: false, execute: false },
    );
}
