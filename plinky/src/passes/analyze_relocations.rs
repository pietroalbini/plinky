use crate::interner::intern;
use crate::repr::object::Object;
use crate::repr::relocations::{Relocation, RelocationType};
use crate::repr::sections::SectionContent;
use crate::repr::symbols::{MissingGlobalSymbol, SymbolId};
use std::collections::BTreeSet;

pub(crate) fn run(object: &Object) -> RelocsAnalysis {
    let mut analysis = RelocsAnalysis { got: None, got_plt: None };

    for section in object.sections.iter() {
        let SectionContent::Data(data) = &section.content else { continue };
        for relocation in &data.relocations {
            match needs_got_entry(relocation.type_) {
                NeedsGot::Got => add_got_reloc(&mut analysis.got, relocation),
                NeedsGot::GotPlt => add_got_reloc(&mut analysis.got_plt, relocation),
                NeedsGot::None => {}
            }
            // Some relocations (like R_386_GOTOFF) require a GOT to be present even if no entries
            // in the GOT are actually there. We thus do the check separately, rather than only
            // emitting the GOT if there are GOT entries.
            match needs_got_table(relocation.type_) {
                NeedsGot::Got => ensure_got(&mut analysis.got),
                NeedsGot::GotPlt => ensure_got(&mut analysis.got_plt),
                NeedsGot::None => {}
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

fn add_got_reloc(got: &mut Option<PlannedGot>, relocation: &Relocation) {
    got.get_or_insert_default().symbols.insert(relocation.symbol);
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

fn needs_got_table(type_: RelocationType) -> NeedsGot {
    match type_ {
        RelocationType::OffsetFromGOT32 => NeedsGot::Got,
        RelocationType::GOTLocationRelative32 => NeedsGot::Got,
        _ => needs_got_entry(type_),
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
    pub(crate) symbols: BTreeSet<SymbolId>,
}
