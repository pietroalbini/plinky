mod sysv_hash;

use super::symbols::SymbolTableKind;
use crate::passes::build_elf::dynamic::sysv_hash::create_sysv_hash;
use crate::passes::build_elf::relocations::create_rela;
use crate::passes::build_elf::symbols::create_symbols;
use crate::passes::build_elf::ElfBuilder;
use crate::passes::layout::SegmentType;
use plinky_elf::ElfPermissions;

macro_rules! add_section {
    ($builder:expr, $segment:expr, $name:expr, $content:expr) => {
        add_section!($builder, $segment, $name, $content, $builder.ids.allocate_section_id());
    };
    ($builder:expr, $segment:expr, $name:expr, $content:expr, $id:expr) => {
        let content = $content;
        let len = content.content_size($builder.object.env.class);
        let old_id = $builder.old_ids.allocate_section_id();
        $builder
            .sections
            .create($name, content)
            .layout(&$segment.add_section(old_id, len as _))
            .old_id(old_id)
            .add_with_id($id);
    };
}

pub(crate) fn add(builder: &mut ElfBuilder) {
    let mut segment = builder.layout.prepare_segment();

    let symbols = create_symbols(
        builder.object.symbols.iter_dynamic_symbols(),
        builder.object.symbols.null_symbol_id(),
        &mut builder.ids,
        &mut builder.sections,
        SymbolTableKind::DynSym,
    );
    let dynsym = builder.ids.allocate_section_id();
    add_section!(builder, segment, ".dynstr", symbols.string_table, symbols.string_table_id);
    add_section!(builder, segment, ".dynsym", symbols.symbol_table, dynsym);

    add_section!(
        builder,
        segment,
        ".rela.dyn",
        create_rela(
            builder.object.dynamic_relocations.iter(),
            builder.object.env.class,
            builder.sections.zero_id,
            dynsym,
            &symbols.conversion,
        )
    );

    add_section!(
        builder,
        segment,
        ".hash",
        create_sysv_hash(
            builder.object.symbols.iter_dynamic_symbols().map(|(_id, sym)| sym),
            dynsym,
        )
    );

    segment.finalize(
        SegmentType::Program,
        ElfPermissions { read: true, write: false, execute: false },
    );
}
