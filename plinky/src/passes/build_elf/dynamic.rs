use crate::passes::build_elf::ElfBuilder;
use crate::repr::dynamic_entries::DynamicEntry;
use crate::repr::relocations::RelocationMode;
use crate::repr::sections::{DynamicSection, SectionId, UpcomingStringId};
use plinky_elf::raw::{RawRel, RawRela, RawSymbol};
use plinky_elf::writer::layout::PartMemory;
use plinky_elf::{ElfDynamic, ElfDynamicDirective, ElfPLTRelocationsMode, ElfSectionContent};
use plinky_utils::Bits;
use plinky_utils::ints::ExtractNumber;
use plinky_utils::raw_types::RawType;

pub(super) fn build_dynamic_section(
    builder: &mut ElfBuilder,
    dynamic: &DynamicSection,
) -> ElfSectionContent {
    let bits: Bits = builder.object.env.class.into();

    let mut string_table_id = None;
    for entry in builder.object.dynamic_entries.iter() {
        match entry {
            DynamicEntry::StringTable(section_id) => string_table_id = Some(*section_id),
            _ => {}
        }
    }

    let mut directives = Vec::new();
    for entry in builder.object.dynamic_entries.iter() {
        let old_len = directives.len();
        match entry {
            DynamicEntry::Needed(string_id) => directives.push(ElfDynamicDirective::Needed {
                string_table_offset: string_offset(builder, string_table_id, *string_id),
            }),

            DynamicEntry::SharedObjectName(string_id) => {
                directives.push(ElfDynamicDirective::SharedObjectName {
                    string_table_offset: string_offset(builder, string_table_id, *string_id),
                })
            }
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
            DynamicEntry::GnuHash(id) => {
                let mem = layout(builder, id, "gnu hash");
                directives.push(ElfDynamicDirective::GnuHash { address: mem.address.extract() });
            }
            DynamicEntry::GotReloc(id) => {
                let mem = layout(builder, id, "got relocations table");
                match builder.object.relocation_mode() {
                    RelocationMode::Rel => {
                        directives
                            .push(ElfDynamicDirective::Rel { address: mem.address.extract() });
                        directives.push(ElfDynamicDirective::RelSize { bytes: mem.len.extract() });
                        directives.push(ElfDynamicDirective::RelEntrySize {
                            bytes: RawRel::size(bits) as _,
                        });
                    }
                    RelocationMode::Rela => {
                        directives
                            .push(ElfDynamicDirective::Rela { address: mem.address.extract() });
                        directives.push(ElfDynamicDirective::RelaSize { bytes: mem.len.extract() });
                        directives.push(ElfDynamicDirective::RelaEntrySize {
                            bytes: RawRela::size(bits) as _,
                        });
                    }
                }
            }
            DynamicEntry::Plt { got_plt, reloc } => {
                let got_plt_mem = layout(builder, got_plt, "got.plt");
                let reloc_mem = layout(builder, reloc, "got.plt's relocations table");
                directives
                    .push(ElfDynamicDirective::JumpRel { address: reloc_mem.address.extract() });
                directives.push(ElfDynamicDirective::PLTRelocationsSize {
                    bytes: reloc_mem.len.extract(),
                });
                directives.push(ElfDynamicDirective::PTLRelocationsMode {
                    mode: match builder.object.relocation_mode() {
                        RelocationMode::Rel => ElfPLTRelocationsMode::Rel,
                        RelocationMode::Rela => ElfPLTRelocationsMode::Rela,
                    },
                });
                directives
                    .push(ElfDynamicDirective::PLTGOT { address: got_plt_mem.address.extract() });
            }
            DynamicEntry::Flags => {
                directives
                    .push(ElfDynamicDirective::Flags(builder.object.dynamic_entries.flags.clone()));
            }
            DynamicEntry::Flags1 => {
                directives.push(ElfDynamicDirective::Flags1(
                    builder.object.dynamic_entries.flags1.clone(),
                ));
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

fn string_offset(builder: &ElfBuilder, table_id: Option<SectionId>, id: UpcomingStringId) -> u64 {
    builder
        .pending_string_tables
        .get(&table_id.expect("no dynamic string table"))
        .expect("dynamic string table not prepared")
        .custom_strings
        .get(&id)
        .expect("missing string in the dynamic string table")
        .offset
        .into()
}

fn layout<'a>(builder: &'a ElfBuilder, id: &SectionId, what: &str) -> &'a PartMemory {
    match &builder.layout.metadata_of_section(id).memory {
        Some(memory) => memory,
        None => panic!("non-allocated {what}"),
    }
}
