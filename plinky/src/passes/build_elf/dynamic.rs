use crate::passes::build_elf::ids::BuiltElfIds;
use crate::passes::build_elf::ElfBuilder;
use crate::repr::dynamic_entries::DynamicEntry;
use crate::repr::sections::DynamicSection;
use plinky_elf::ids::serial::SectionId;
use plinky_elf::raw::{RawRela, RawSymbol};
use plinky_elf::writer::layout::PartMemory;
use plinky_elf::{
    ElfDynamic, ElfDynamicDirective, ElfDynamicFlags1, ElfPLTRelocationsMode, ElfSectionContent,
};
use plinky_utils::ints::ExtractNumber;
use plinky_utils::raw_types::RawType;

pub(super) fn build_dynamic_section(
    builder: &mut ElfBuilder,
    dynamic: &DynamicSection,
) -> ElfSectionContent<BuiltElfIds> {
    let bits = builder.object.env.class;

    let mut directives = Vec::new();
    for entry in builder.object.dynamic_entries.iter() {
        let old_len = directives.len();
        match entry {
            DynamicEntry::StringTable(id) => {
                let mem = layout(builder, id, "dynamic string table");
                directives
                    .push(ElfDynamicDirective::StringTable { address: mem.address.extract() });
                directives.push(ElfDynamicDirective::StringTableSize { bytes: mem.len.extract() });
            }
            DynamicEntry::SymbolTable(id) => {
                let mem = layout(builder, id, "dynamic symbol table");
                directives
                    .push(ElfDynamicDirective::SymbolTable { address: mem.address.extract() });
                directives.push(ElfDynamicDirective::SymbolTableEntrySize {
                    bytes: RawSymbol::size(bits) as _,
                });
            }
            DynamicEntry::Hash(id) => {
                let mem = layout(builder, id, "sysv hash");
                directives.push(ElfDynamicDirective::Hash { address: mem.address.extract() });
            }
            DynamicEntry::GotRela(id) => {
                let mem = layout(builder, id, "got relocations table");
                directives.push(ElfDynamicDirective::Rela { address: mem.address.extract() });
                directives.push(ElfDynamicDirective::RelaSize { bytes: mem.len.extract() });
                directives
                    .push(ElfDynamicDirective::RelaEntrySize { bytes: RawRela::size(bits) as _ });
            }
            DynamicEntry::Plt { got_plt, rela } => {
                let got_plt_mem = layout(builder, got_plt, "got.plt relocations table");
                let rela_mem = layout(builder, rela, "rela.plt relocations table");
                directives
                    .push(ElfDynamicDirective::JumpRel { address: rela_mem.address.extract() });
                directives.push(ElfDynamicDirective::PLTRelocationsSize {
                    bytes: rela_mem.len.extract(),
                });
                directives.push(ElfDynamicDirective::PTLRelocationsMode {
                    mode: ElfPLTRelocationsMode::Rela,
                });
                directives
                    .push(ElfDynamicDirective::PLTGOT { address: got_plt_mem.address.extract() });
            }
            DynamicEntry::PieFlag => {
                directives.push(ElfDynamicDirective::Flags1(ElfDynamicFlags1 { pie: true }));
            }
        }

        // Ensure the implmenetation of directives_count() is not wrong.
        assert_eq!(entry.directives_count(), directives.len() - old_len);
    }

    // Section must be null-terminated.
    directives.push(ElfDynamicDirective::Null);

    ElfSectionContent::Dynamic(ElfDynamic {
        string_table: *builder.section_ids.get(&dynamic.strings()).unwrap(),
        directives,
    })
}

fn layout<'a>(builder: &'a ElfBuilder, id: &SectionId, what: &str) -> &'a PartMemory {
    match &builder.layout.metadata_of_section(id).memory {
        Some(memory) => memory,
        None => panic!("non-allocated {what}"),
    }
}
