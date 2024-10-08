use crate::cli::CliOptions;
use crate::interner::intern;
use crate::passes::generate_dynamic::DynamicContext;
use crate::repr::dynamic_entries::DynamicEntry;
use crate::repr::object::Object;
use crate::repr::relocations::{NeedsGot, Relocation, RelocationType};
use crate::repr::sections::{DataSection, RelocationsSection, SectionContent, SectionId};
use crate::repr::segments::SegmentContent;
use crate::repr::symbols::views::AllSymbols;
use crate::repr::symbols::{LoadSymbolsError, SymbolId, SymbolValue, UpcomingSymbol};
use plinky_elf::{ElfClass, ElfPermissions};
use plinky_macros::{Display, Error};
use plinky_utils::ints::Offset;
use std::collections::{BTreeMap, BTreeSet};

pub(crate) fn generate_got(
    options: &CliOptions,
    object: &mut Object,
    dynamic_context: &Option<DynamicContext>,
) -> Result<(), GenerateGotError> {
    let mut got_needed = false;
    let mut got_symbols = BTreeSet::new();
    let mut got_plt_needed = false;
    let mut got_plt_symbols = BTreeSet::new();
    for section in object.sections.iter() {
        let SectionContent::Data(data) = &section.content else {
            continue;
        };
        for relocation in &data.relocations {
            match relocation.type_.needs_got_entry() {
                NeedsGot::None => false, // Empty block, false due to insert() returning a bool.
                NeedsGot::Got => got_symbols.insert(relocation.symbol),
                NeedsGot::GotPlt => got_plt_symbols.insert(relocation.symbol),
            };

            // Some relocations (like R_386_GOTOFF) require a GOT to be present even if no entries
            // in the GOT are actually there. We thus do the check separately, rather than only
            // emitting the GOT if there are GOT entries.
            match relocation.type_.needs_got_table() {
                NeedsGot::None => {}
                NeedsGot::Got => got_needed = true,
                NeedsGot::GotPlt => got_plt_needed = true,
            }
        }
    }

    let got_symbol = intern("_GLOBAL_OFFSET_TABLE_");
    got_plt_needed |=
        object.symbols.iter(&AllSymbols).filter(|s| s.name() == got_symbol).next().is_some();

    if got_needed {
        object.got = Some(build_got(
            object,
            dynamic_context,
            &mut got_symbols.iter().copied(),
            GotConfig {
                section_name: ".got",
                rela_section_name: ".rela.got",
                inside_relro: options.read_only_got,
                add_prelude: false,
                relocation_type: RelocationType::FillGotSlot,
                dynamic_entry: |_got_plt, rela| DynamicEntry::GotRela(rela),
            },
        )?);
    }

    if got_plt_needed {
        let mut got_plt = build_got(
            object,
            dynamic_context,
            &mut got_plt_symbols.iter().copied(),
            GotConfig {
                section_name: ".got.plt",
                rela_section_name: ".rela.plt",
                inside_relro: options.read_only_got_plt,
                add_prelude: true,
                relocation_type: RelocationType::FillGotPltSlot,
                dynamic_entry: |got_plt, rela| DynamicEntry::Plt { got_plt, rela },
            },
        )?;

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
    symbols: &mut dyn Iterator<Item = SymbolId>,
    config: GotConfig,
) -> Result<GOT, GenerateGotError> {
    let mut buf = Vec::new();
    let mut relocations = Vec::new();
    let mut offsets = BTreeMap::new();

    let id = object.sections.reserve_placeholder();
    let placeholder: &[u8] = match object.env.class {
        ElfClass::Elf32 => &[0; 4],
        ElfClass::Elf64 => &[0; 8],
    };

    // The psABI for x86-64 states that the first entry in the .got.plt must point to the _DYNAMIC
    // symbol (resolved at link time), and it must be followed by two other entries reserved for
    // the use of the dynamic linker.
    let mut prelude_relocation = None;
    if config.add_prelude {
        if let Some(dynamic) = dynamic_context {
            for _ in 0..3 {
                buf.extend_from_slice(placeholder);
            }
            prelude_relocation = Some(Relocation {
                type_: RelocationType::Absolute32,
                symbol: dynamic.dynamic_symbol(),
                offset: 0.into(),
                addend: Some(0.into()),
            });
        }
    }

    for symbol in symbols {
        let offset =
            Offset::from(i64::try_from(buf.len()).map_err(|_| GenerateGotError::TooLarge)?);

        buf.extend_from_slice(placeholder);
        relocations.push(Relocation {
            type_: config.relocation_type,
            symbol,
            offset,
            addend: Some(0.into()),
        });
        offsets.insert(symbol, offset);
    }

    let mut data = DataSection::new(ElfPermissions::RW, &buf);
    data.inside_relro = config.inside_relro;

    // The relocation for the _DYNAMIC symbol in the prelude must always be resolved at link time,
    // so we unconditionally add it in the relocations applied by the linker.
    if let Some(relocation) = prelude_relocation {
        data.relocations.push(relocation);
    }

    let resolved_at_runtime = match dynamic_context {
        Some(dynamic) => {
            if !relocations.is_empty() {
                for relocation in &relocations {
                    object.symbols.get_mut(relocation.symbol).mark_needed_by_dynamic();
                }

                let rela = object
                    .sections
                    .builder(
                        config.rela_section_name,
                        RelocationsSection::new(id, dynamic.dynsym(), relocations),
                    )
                    .create();
                object
                    .segments
                    .get_mut(dynamic.segment())
                    .content
                    .push(SegmentContent::Section(rela));
                object.dynamic_entries.add((config.dynamic_entry)(id, rela));

                true
            } else {
                false
            }
        }
        None => {
            data.relocations.extend(relocations.into_iter());
            false
        }
    };

    object.sections.builder(config.section_name, data).create_in_placeholder(id);
    Ok(GOT { id, offsets, symbol: None, resolved_at_runtime })
}

struct GotConfig {
    section_name: &'static str,
    rela_section_name: &'static str,
    inside_relro: bool,
    add_prelude: bool,
    relocation_type: RelocationType,
    dynamic_entry: fn(SectionId, SectionId) -> DynamicEntry,
}

#[derive(Debug)]
pub(crate) struct GOT {
    pub(crate) id: SectionId,
    pub(crate) offsets: BTreeMap<SymbolId, Offset>,
    pub(crate) symbol: Option<SymbolId>,
    pub(crate) resolved_at_runtime: bool,
}

impl GOT {
    pub(crate) fn offset(&self, symbol: SymbolId) -> Offset {
        match self.offsets.get(&symbol) {
            Some(offset) => *offset,
            None => panic!("did not generate a got entry for {symbol:?}"),
        }
    }
}

#[derive(Debug, Display, Error)]
pub(crate) enum GenerateGotError {
    #[display("the GOT is too large")]
    TooLarge,
    #[display("failed to create the _GLOBAL_OFFSET_TABLE_ symbol")]
    CreateSymbol(#[source] LoadSymbolsError),
}
