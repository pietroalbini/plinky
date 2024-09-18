use crate::interner::intern;
use crate::passes::prepare_dynamic::DynamicContext;
use crate::repr::dynamic_entries::DynamicEntry;
use crate::repr::object::Object;
use crate::repr::relocations::{Relocation, RelocationType};
use crate::repr::sections::{DataSection, RelocationsSection, SectionContent};
use crate::repr::segments::SegmentContent;
use crate::repr::symbols::{LoadSymbolsError, Symbol, SymbolValue};
use plinky_elf::ids::serial::{SectionId, SerialIds, SymbolId};
use plinky_elf::{ElfClass, ElfPermissions};
use plinky_macros::{Display, Error};
use plinky_utils::ints::Offset;
use std::collections::{BTreeMap, BTreeSet};

pub(crate) fn generate_got(
    ids: &mut SerialIds,
    object: &mut Object,
    dynamic_context: &Option<DynamicContext>,
) -> Result<(), GenerateGotError> {
    let mut needs_got = false;
    let mut symbols = BTreeSet::new();
    for section in object.sections.iter() {
        let SectionContent::Data(data) = &section.content else {
            continue;
        };
        for relocation in &data.relocations {
            if relocation.type_.needs_got_entry() {
                symbols.insert(relocation.symbol);
            }
            // Some relocations (like R_386_GOTOFF) require a GOT to be present even if no entries
            // in the GOT are actually there. We thus do the check separately, rather than only
            // emitting the GOT if there are GOT entries.
            needs_got |= relocation.type_.needs_got_table();
        }
    }

    if !needs_got {
        return Ok(());
    }

    let got = build_got(
        ids,
        object,
        dynamic_context,
        &mut symbols.iter().copied(),
        GotConfig {
            section_name: ".got",
            rela_section_name: ".rela.got",
            inside_relro: true,
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
            type_: RelocationType::FillGOTSlot,
            symbol,
            section: id,
            offset,
            addend: Some(0.into()),
        });
        offsets.insert(symbol, offset);
    }

    let mut data = DataSection::new(ElfPermissions::empty().read().write(), &buf);
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
