use crate::cli::CliOptions;
use crate::interner::intern;
use crate::passes::prepare_dynamic::DynamicContext;
use crate::repr::dynamic_entries::DynamicEntry;
use crate::repr::object::Object;
use crate::repr::relocations::{NeedsGot, Relocation, RelocationType};
use crate::repr::sections::{DataSection, RelocationsSection, SectionContent};
use crate::repr::segments::SegmentContent;
use crate::repr::symbols::{LoadSymbolsError, Symbol, SymbolValue};
use plinky_elf::ids::serial::{SectionId, SerialIds, SymbolId};
use plinky_elf::{ElfClass, ElfPermissions};
use plinky_macros::{Display, Error};
use plinky_utils::ints::Offset;
use std::collections::{BTreeMap, BTreeSet};

pub(crate) fn generate_got(
    options: &CliOptions,
    ids: &mut SerialIds,
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

    if got_needed {
        let got = build_got(
            ids,
            object,
            dynamic_context,
            &mut got_symbols.iter().copied(),
            GotConfig {
                section_name: ".got",
                rela_section_name: ".rela.got",
                inside_relro: options.read_only_got,
                relocation_type: RelocationType::FillGotSlot,
                dynamic_entry: DynamicEntry::Rela,
            },
        )?;

        let got_symbol = ids.allocate_symbol_id();
        object
            .symbols
            .add_symbol(Symbol::new_global_hidden(
                got_symbol,
                intern("_GLOBAL_OFFSET_TABLE_"),
                SymbolValue::SectionRelative { section: got.id, offset: 0.into() },
            ))
            .map_err(GenerateGotError::CreateSymbol)?;

        object.got = Some(got);
    }

    if got_plt_needed {
        build_got(
            ids,
            object,
            dynamic_context,
            &mut got_plt_symbols.iter().copied(),
            GotConfig {
                section_name: ".got.plt",
                rela_section_name: ".rela.plt",
                inside_relro: false, // TODO: depending on -znow
                relocation_type: RelocationType::FillGotPltSlot,
                dynamic_entry: DynamicEntry::Rela, // TODO: use correct dynamic entry
            },
        )?;
    }

    // TODO: _GLOBAL_OFFSET_TABLE_ should be .got.plt, not .got (also changing the x86 relocation
    // needing a got to require .got.plt instead of .got)

    Ok(())
}

fn build_got(
    ids: &mut SerialIds,
    object: &mut Object,
    dynamic_context: &Option<DynamicContext>,
    symbols: &mut dyn Iterator<Item = SymbolId>,
    config: GotConfig,
) -> Result<GOT, GenerateGotError> {
    let mut buf = Vec::new();
    let mut relocations = Vec::new();
    let mut offsets = BTreeMap::new();

    let id = ids.allocate_section_id();
    let placeholder: &[u8] = match object.env.class {
        ElfClass::Elf32 => &[0; 4],
        ElfClass::Elf64 => &[0; 8],
    };

    for symbol in symbols {
        let offset =
            Offset::from(i64::try_from(buf.len()).map_err(|_| GenerateGotError::TooLarge)?);

        buf.extend_from_slice(placeholder);
        relocations.push(Relocation {
            type_: config.relocation_type,
            symbol,
            section: id,
            offset,
            addend: Some(0.into()),
        });
        offsets.insert(symbol, offset);
    }

    let mut data = DataSection::new(ElfPermissions::RW, &buf);
    data.inside_relro = config.inside_relro;

    match dynamic_context {
        Some(dynamic) => {
            for relocation in &relocations {
                object.symbols.get_mut(relocation.symbol).mark_needed_by_dynamic();
            }

            let rela = object
                .sections
                .builder(
                    config.rela_section_name,
                    RelocationsSection::new(None, dynamic.dynsym(), relocations),
                )
                .create(ids);
            object.segments.get_mut(dynamic.segment()).content.push(SegmentContent::Section(rela));
            object.dynamic_entries.add((config.dynamic_entry)(rela));
        }
        None => data.relocations = relocations,
    }

    object.sections.builder(config.section_name, data).create_with_id(id);
    Ok(GOT { id, offsets })
}

struct GotConfig {
    section_name: &'static str,
    rela_section_name: &'static str,
    inside_relro: bool,
    relocation_type: RelocationType,
    dynamic_entry: fn(SectionId) -> DynamicEntry,
}

#[derive(Debug)]
pub(crate) struct GOT {
    pub(crate) id: SectionId,
    pub(crate) offsets: BTreeMap<SymbolId, Offset>,
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
