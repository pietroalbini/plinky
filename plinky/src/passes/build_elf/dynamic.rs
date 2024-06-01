use crate::passes::build_elf::relocations::add_rela;
use crate::passes::build_elf::symbols::add_symbols;
use crate::passes::build_elf::ElfBuilder;

pub(crate) fn add(builder: &mut ElfBuilder<'_>) {
    let dynsym =
        add_symbols(builder, ".dynsym", ".dynstr", |symbols| symbols.iter_dynamic_symbols());

    add_rela(builder, ".rela.dyn", &dynsym, |object| object.dynamic_relocations.iter());
}
