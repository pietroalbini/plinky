use crate::repr::object::Object;
use crate::repr::relocations::{Relocation, RelocationType};
use crate::repr::sections::{DataSection, SectionContent};
use crate::utils::ints::Offset;
use plinky_elf::ids::serial::{SectionId, SerialIds, SymbolId};
use plinky_elf::ElfPermissions;
use std::collections::{BTreeMap, BTreeSet};

pub(crate) fn generate_got(ids: &mut SerialIds, object: &mut Object) {
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
        return;
    }

    let id = ids.allocate_section_id();

    let (placeholder, placeholder_len): (&[u8], i64) = match object.env.class {
        plinky_elf::ElfClass::Elf32 => (&[0; 4], 4),
        plinky_elf::ElfClass::Elf64 => (&[0; 8], 8),
    };

    let mut bytes = Vec::new();
    let mut relocations = Vec::new();
    let mut offsets = BTreeMap::new();
    let mut current_offset = 0;
    for symbol in symbols {
        let offset = Offset::from(current_offset);
        current_offset += placeholder_len;

        bytes.extend_from_slice(placeholder);
        relocations.push(Relocation {
            type_: RelocationType::FillGOTSlot,
            symbol,
            section: id,
            offset,
            addend: Some(0.into()),
        });
        offsets.insert(symbol, offset);
    }

    let mut data = DataSection::new(ElfPermissions::empty().read().write(), &bytes);
    data.relocations = relocations;
    object.sections.builder(".got", data).create_with_id(id);

    object.got = Some(GOT { id, offsets });
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
