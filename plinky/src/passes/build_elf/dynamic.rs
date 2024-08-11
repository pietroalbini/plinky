use crate::passes::build_elf::ids::BuiltElfIds;
use crate::passes::build_elf::ElfBuilder;
use crate::passes::layout::SectionLayout;
use crate::repr::dynamic_entries::DynamicEntry;
use crate::repr::sections::DynamicSection;
use plinky_utils::ints::ExtractNumber;
use plinky_elf::raw::{RawRela, RawSymbol};
use plinky_elf::{ElfDynamic, ElfDynamicDirective, ElfDynamicFlags1, ElfSectionContent};
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
                let SectionLayout::Allocated { address, len } = builder.layout.of_section(*id)
                else {
                    panic!("non-allocated dynamic string table");
                };
                directives.push(ElfDynamicDirective::StringTable { address: address.extract() });
                directives.push(ElfDynamicDirective::StringTableSize { bytes: len.extract() });
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
                directives.push(ElfDynamicDirective::RelaSize { bytes: len.extract() });
                directives
                    .push(ElfDynamicDirective::RelaEntrySize { bytes: RawRela::size(bits) as _ });
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
        string_table: builder.sections.new_id_of(dynamic.strings()),
        directives,
    })
}
