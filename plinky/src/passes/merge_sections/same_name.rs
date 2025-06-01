use crate::interner::{Interned, intern};
use crate::repr::object::Object;
use crate::repr::sections::{DataSection, SectionContent, SectionId};
use plinky_diagnostics::ObjectSpan;
use plinky_elf::{ElfDeduplication, ElfPermissions};
use plinky_macros::{Display, Error};
use plinky_utils::ints::{Length, Offset, OutOfBoundsError};
use std::collections::BTreeMap;

pub(super) fn run(
    object: &mut Object,
) -> Result<BTreeMap<SectionId, SameNameMerge>, MergeSameNameError> {
    let mut grouped_sections: BTreeMap<_, Vec<_>> = BTreeMap::new();
    for section in object.sections.iter() {
        let SectionContent::Data(data) = &section.content else { continue };
        grouped_sections
            .entry(GroupKey {
                name: section.name,
                perms: data.perms,
                deduplication: data.deduplication,
                inside_relro: data.inside_relro,
            })
            .or_default()
            .push(GroupValue { id: section.id, source: section.source.clone() });
    }
    grouped_sections.retain(|_name, sections| sections.len() > 1);

    let mut merged = BTreeMap::new();
    for (key, sections) in grouped_sections {
        let id = object.sections.reserve_placeholder();
        let mut data = DataSection {
            perms: key.perms,
            deduplication: key.deduplication,
            bytes: Vec::new(),
            relocations: Vec::new(),
            inside_relro: key.inside_relro,
        };

        let mut sections_iter = sections.iter();
        let mut source = sections_iter.next().unwrap().source.clone();
        for old in sections_iter {
            source = source.merge(&old.source);
        }
        let interned_source = intern(source.clone());

        let mut offset = Offset::from(0);
        let mut is_retain = false;
        for old in sections {
            let old_section = object.sections.remove(old.id, None);
            let SectionContent::Data(old_data) = old_section.content else {
                panic!("only data sections should reach here");
            };

            data.bytes.extend_from_slice(&old_data.bytes);
            for mut relocation in old_data.relocations {
                relocation.offset = relocation.offset.add(offset)?;
                data.relocations.push(relocation);
            }

            if old_section.retain {
                is_retain = true;
            }

            merged.insert(old.id, SameNameMerge { target: id, offset, span: interned_source });
            offset = Length::from(data.bytes.len()).as_offset()?;
        }

        object
            .sections
            .builder(key.name.resolve().as_str(), data)
            .source(source)
            .retain(is_retain)
            .create_in_placeholder(id);
    }

    Ok(merged)
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct GroupKey {
    name: Interned<String>,
    perms: ElfPermissions,
    deduplication: ElfDeduplication,
    inside_relro: bool,
}

struct GroupValue {
    id: SectionId,
    source: ObjectSpan,
}

#[derive(Debug)]
pub(super) struct SameNameMerge {
    pub(super) target: SectionId,
    pub(super) offset: Offset,
    pub(super) span: Interned<ObjectSpan>,
}

#[derive(Debug, Error, Display)]
pub(crate) enum MergeSameNameError {
    #[transparent]
    OutOfBounds(OutOfBoundsError),
}
