use crate::cli::Mode;
use crate::interner::intern;
use crate::repr::object::Object;
use crate::repr::relocations::{Relocation, RelocationType};
use crate::repr::sections::SectionContent;
use crate::repr::symbols::{MissingGlobalSymbol, SymbolId, SymbolValue};
use std::collections::{btree_map, BTreeMap};

pub(crate) fn run(object: &Object) -> RelocsAnalysis {
    let mut analysis = RelocsAnalysis { got: None, got_plt: None };

    for section in object.sections.iter() {
        let SectionContent::Data(data) = &section.content else { continue };
        for relocation in &data.relocations {
            match needs_got_entry(relocation.type_) {
                NeedsGot::Got => add_got_reloc(&mut analysis.got, object, relocation),
                NeedsGot::GotPlt => add_got_reloc(&mut analysis.got_plt, object, relocation),
                NeedsGot::None => {}
            }
            // Some relocations (like R_386_GOTOFF or R_386_GOT32) require a .got.plt to be present
            // even if no PLT entries are actually present. This is because they are relative to
            // _GLOBAL_OFFSET_TABLE_ symbol, which points at .got.plt on x86 and x86-64. We need to
            // ensure .got.plt is present if anything references that symbol.
            if needs_got_symbol(relocation.type_) {
                ensure_got(&mut analysis.got_plt);
            }
        }
    }

    // If something refers directly to the _GLOBAL_OFFSET_TABLE_ we need to ensure it's present.
    match object.symbols.get_global(intern("_GLOBAL_OFFSET_TABLE_")) {
        Ok(_) => ensure_got(&mut analysis.got_plt),
        Err(MissingGlobalSymbol { .. }) => {}
    }

    analysis
}

fn ensure_got(got: &mut Option<PlannedGot>) {
    if got.is_none() {
        *got = Some(PlannedGot::default());
    }
}

fn add_got_reloc(got: &mut Option<PlannedGot>, object: &Object, relocation: &Relocation) {
    let resolved_at = match object.mode {
        Mode::PositionDependent => match object.symbols.get(relocation.symbol).value() {
            SymbolValue::ExternallyDefined => ResolvedAt::RunTime,
            _ => ResolvedAt::LinkTime,
        },
        Mode::PositionIndependent | Mode::SharedLibrary => ResolvedAt::RunTime,
    };

    match got.get_or_insert_default().symbols.entry(relocation.symbol) {
        btree_map::Entry::Vacant(entry) => {
            entry.insert(PlannedGotSymbol { id: relocation.symbol, resolved_at });
        }
        btree_map::Entry::Occupied(mut entry) => {
            let old_entry = entry.get();
            entry.insert(PlannedGotSymbol {
                id: old_entry.id,
                resolved_at: old_entry.resolved_at.max(resolved_at),
            });
        }
    }
}

fn needs_got_entry(type_: RelocationType) -> NeedsGot {
    match type_ {
        RelocationType::Absolute32 => NeedsGot::None,
        RelocationType::AbsoluteSigned32 => NeedsGot::None,
        RelocationType::Relative32 => NeedsGot::None,
        RelocationType::PLT32 => NeedsGot::GotPlt,
        RelocationType::GOTRelative32 => NeedsGot::Got,
        RelocationType::GOTIndex32 => NeedsGot::Got,
        RelocationType::GOTLocationRelative32 => NeedsGot::None,
        RelocationType::OffsetFromGOT32 => NeedsGot::None,
        RelocationType::FillGotSlot => NeedsGot::None,
        RelocationType::FillGotPltSlot => NeedsGot::None,
    }
}

fn needs_got_symbol(type_: RelocationType) -> bool {
    match type_ {
        RelocationType::GOTIndex32
        | RelocationType::GOTLocationRelative32
        | RelocationType::OffsetFromGOT32 => true,
        RelocationType::Absolute32
        | RelocationType::AbsoluteSigned32
        | RelocationType::Relative32
        | RelocationType::PLT32
        | RelocationType::GOTRelative32
        | RelocationType::FillGotSlot
        | RelocationType::FillGotPltSlot => false,
    }
}

enum NeedsGot {
    Got,
    GotPlt,
    None,
}

pub(crate) struct RelocsAnalysis {
    pub(crate) got: Option<PlannedGot>,
    pub(crate) got_plt: Option<PlannedGot>,
}

#[derive(Default)]
pub(crate) struct PlannedGot {
    symbols: BTreeMap<SymbolId, PlannedGotSymbol>,
}

impl PlannedGot {
    pub(crate) fn symbols(&self) -> impl Iterator<Item = &PlannedGotSymbol> {
        self.symbols.values()
    }
}

#[derive(Debug)]
pub(crate) struct PlannedGotSymbol {
    pub(crate) id: SymbolId,
    pub(crate) resolved_at: ResolvedAt,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub(crate) enum ResolvedAt {
    // The order of the members here is important: when a symbol is present in multiple relocs, and
    // different relocations should be resolved at different times, the variant defined lower in
    // this enum will have precedence.
    LinkTime,
    RunTime,
}
