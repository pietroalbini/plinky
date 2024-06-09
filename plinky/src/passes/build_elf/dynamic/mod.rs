mod sysv_hash;

use super::symbols::SymbolTableKind;
use crate::passes::build_elf::dynamic::sysv_hash::create_sysv_hash;
use crate::passes::build_elf::relocations::create_rela;
use crate::passes::build_elf::symbols::create_symbols;
use crate::passes::build_elf::ElfBuilder;

pub(crate) fn add(builder: &mut ElfBuilder) {
    let symbols = create_symbols(
        builder.object.symbols.iter_dynamic_symbols(),
        builder.object.symbols.null_symbol_id(),
        &mut builder.ids,
        &mut builder.sections,
        SymbolTableKind::DynSym,
    );
    let dynsym = builder.sections.create(".dynsym", symbols.symbol_table).add(&mut builder.ids);
    builder.sections.create(".dynstr", symbols.string_table).add_with_id(symbols.string_table_id);

    let rela = create_rela(
        builder.object.dynamic_relocations.iter(),
        builder.object.env.class,
        builder.sections.zero_id,
        dynsym,
        &symbols.conversion,
    );
    builder.sections.create(".rela.dyn", rela).add(&mut builder.ids);

    let hash = create_sysv_hash(
        builder.object.symbols.iter_dynamic_symbols().map(|(_id, sym)| sym),
        dynsym,
    );
    builder.sections.create(".hash", hash).add(&mut builder.ids);
}
