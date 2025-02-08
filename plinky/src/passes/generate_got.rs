use super::analyze_relocations::ResolvedAt;
use crate::cli::CliOptions;
use crate::interner::intern;
use crate::passes::analyze_relocations::{PlannedGotSymbol, RelocsAnalysis};
use crate::passes::generate_dynamic::DynamicContext;
use crate::repr::dynamic_entries::DynamicEntry;
use crate::repr::object::Object;
use crate::repr::relocations::{Relocation, RelocationMode, RelocationType};
use crate::repr::sections::{DataSection, RelocationsSection, SectionId};
use crate::repr::segments::SegmentContent;
use crate::repr::symbols::{LoadSymbolsError, SymbolId, SymbolValue, UpcomingSymbol};
use plinky_elf::{ElfClass, ElfPermissions};
use plinky_macros::{Display, Error};
use plinky_utils::ints::Offset;
use std::collections::BTreeMap;

pub(crate) fn generate_got(
    options: &CliOptions,
    object: &mut Object,
    relocs_analysis: &RelocsAnalysis,
    dynamic_context: &Option<DynamicContext>,
) -> Result<(), GenerateGotError> {
    if let Some(plan) = &relocs_analysis.got {
        object.got = Some(build_got(object, dynamic_context, &mut plan.symbols(), GotConfig {
            section_name: ".got",
            reloc_section_name: match object.relocation_mode() {
                RelocationMode::Rel => ".rel.got",
                RelocationMode::Rela => ".rela.got",
            },
            inside_relro: options.read_only_got,
            add_prelude: false,
            relocation_type: RelocationType::FillGotSlot,
            dynamic_entry: |_got_plt, reloc| DynamicEntry::GotReloc(reloc),
        })?);
    }

    if let Some(plan) = &relocs_analysis.got_plt {
        let mut got_plt = build_got(object, dynamic_context, &mut plan.symbols(), GotConfig {
            section_name: ".got.plt",
            reloc_section_name: match object.relocation_mode() {
                RelocationMode::Rel => ".rel.plt",
                RelocationMode::Rela => ".rela.plt",
            },
            inside_relro: options.read_only_got_plt,
            add_prelude: true,
            relocation_type: RelocationType::FillGotPltSlot,
            dynamic_entry: |got_plt, reloc| DynamicEntry::Plt { got_plt, reloc },
        })?;

        if options.read_only_got_plt {
            object.dynamic_entries.flags.bind_now = true;
        }

        let got_plt_symbol = object
            .symbols
            .add(UpcomingSymbol::GlobalHidden {
                name: intern("_GLOBAL_OFFSET_TABLE_"),
                value: SymbolValue::SectionRelative { section: got_plt.id, offset: 0.into() },
            })
            .map_err(GenerateGotError::CreateSymbol)?;

        got_plt.symbol = Some(got_plt_symbol);
        object.got_plt = Some(got_plt);
    }

    Ok(())
}

fn build_got(
    object: &mut Object,
    dynamic_context: &Option<DynamicContext>,
    symbols: &mut dyn Iterator<Item = &PlannedGotSymbol>,
    config: GotConfig,
) -> Result<Got, GenerateGotError> {
    let mut buf = Vec::new();
    let mut entries = BTreeMap::new();
    let mut link_time_relocs = Vec::new();
    let mut run_time_relocs = Vec::new();

    let id = object.sections.reserve_placeholder();
    let placeholder: &[u8] = match object.env.class {
        ElfClass::Elf32 => &[0; 4],
        ElfClass::Elf64 => &[0; 8],
    };

    // The psABI for x86-64 states that the first entry in the .got.plt must point to the _DYNAMIC
    // symbol (resolved at link time), and it must be followed by two other entries reserved for
    // the use of the dynamic linker.
    if config.add_prelude {
        if let Some(dynamic) = dynamic_context {
            for _ in 0..3 {
                buf.extend_from_slice(placeholder);
            }
            // The relocation for the _DYNAMIC symbol in the prelude must always be resolved at
            // link time, so we unconditionally add it in the relocations applied by the linker.
            link_time_relocs.push(Relocation {
                type_: RelocationType::Absolute32,
                symbol: dynamic.dynamic_symbol(),
                offset: 0.into(),
                addend: Offset::from(0).into(),
            });
        }
    }

    for symbol in symbols {
        let offset =
            Offset::from(i64::try_from(buf.len()).map_err(|_| GenerateGotError::TooLarge)?);

        let reloc = Relocation {
            type_: config.relocation_type,
            symbol: symbol.id,
            offset,
            addend: Offset::from(0).into(),
        };

        let mut dynamic_relocation_index = None;
        match symbol.resolved_at {
            ResolvedAt::LinkTime => link_time_relocs.push(reloc),
            ResolvedAt::RunTime => {
                dynamic_relocation_index = Some(run_time_relocs.len());
                run_time_relocs.push(reloc);
            }
        }

        buf.extend_from_slice(placeholder);
        entries.insert(symbol.id, GotEntry {
            offset,
            resolved_at: symbol.resolved_at,
            dynamic_relocation_index,
        });
    }

    let mut data = DataSection::new(ElfPermissions::RW, &buf);
    data.inside_relro = config.inside_relro;

    // These relocations will later be applied by Plinky's relocator.
    data.relocations.extend(link_time_relocs.into_iter());

    // These relocations will be applied by the dynamic linker.
    if !run_time_relocs.is_empty() {
        let Some(dynamic) = dynamic_context else {
            panic!("there are GOT entries to resolve at runtime without a dynamic section");
        };

        for relocation in &run_time_relocs {
            object.symbols.get_mut(relocation.symbol).mark_needed_by_dynamic();
        }

        let reloc = object
            .sections
            .builder(
                config.reloc_section_name,
                RelocationsSection::new(id, dynamic.dynsym(), run_time_relocs),
            )
            .create();
        object.segments.get_mut(dynamic.segment()).content.push(SegmentContent::Section(reloc));
        object.dynamic_entries.add((config.dynamic_entry)(id, reloc));
    }

    object.sections.builder(config.section_name, data).create_in_placeholder(id);
    Ok(Got { id, entries, symbol: None })
}

struct GotConfig {
    section_name: &'static str,
    reloc_section_name: &'static str,
    inside_relro: bool,
    add_prelude: bool,
    relocation_type: RelocationType,
    dynamic_entry: fn(SectionId, SectionId) -> DynamicEntry,
}

#[derive(Debug)]
pub(crate) struct Got {
    pub(crate) id: SectionId,
    pub(crate) entries: BTreeMap<SymbolId, GotEntry>,
    pub(crate) symbol: Option<SymbolId>,
}

impl Got {
    pub(crate) fn offset(&self, symbol: SymbolId) -> Offset {
        match self.entries.get(&symbol) {
            Some(entry) => entry.offset,
            None => panic!("did not generate a got entry for {symbol:?}"),
        }
    }
}

#[derive(Debug)]
pub(crate) struct GotEntry {
    pub(crate) offset: Offset,
    pub(crate) resolved_at: ResolvedAt,
    pub(crate) dynamic_relocation_index: Option<usize>,
}

#[derive(Debug, Display, Error)]
pub(crate) enum GenerateGotError {
    #[display("the GOT is too large")]
    TooLarge,
    #[display("failed to create the _GLOBAL_OFFSET_TABLE_ symbol")]
    CreateSymbol(#[source] LoadSymbolsError),
}
