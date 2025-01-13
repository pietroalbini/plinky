use crate::interner::Interned;
use crate::repr::object::Object;
use crate::repr::sections::{DataSection, SectionContent, SectionId};
use crate::repr::symbols::{LoadSymbolsError, SymbolId, UpcomingSymbol};
use plinky_elf::{ElfDeduplication, ElfPermissions};
use plinky_macros::{Display, Error};
use plinky_utils::ints::{Length, Offset, OutOfBoundsError};
use std::collections::BTreeMap;
use std::num::NonZeroU64;

pub(super) fn run(
    object: &mut Object,
) -> Result<BTreeMap<SectionId, Deduplication>, DeduplicationError> {
    let mut groups: BTreeMap<_, Vec<_>> = BTreeMap::new();
    for section in object.sections.iter() {
        let SectionContent::Data(data) = &section.content else {
            continue;
        };

        let split_rule = match data.deduplication {
            ElfDeduplication::Disabled => continue,
            ElfDeduplication::ZeroTerminatedStrings => SplitRule::ZeroTerminatedString,
            ElfDeduplication::FixedSizeChunks { size } => SplitRule::FixedSizeChunks { size },
        };

        // Not sure exactly whether relocations inside of deduplicatable sections are ever used in
        // the wild, so for now let's error out if we encounter this.
        if !data.relocations.is_empty() {
            return Err(DeduplicationError {
                section_name: section.name,
                kind: DeduplicationErrorKind::RelocationsUnsupported,
            });
        }

        groups.entry((section.name, data.perms, split_rule)).or_default().push(section.id);
    }

    let mut deduplications = BTreeMap::new();
    for ((name, perms, split_rule), section_ids) in groups {
        if section_ids.len() > 1 {
            deduplicate(&mut deduplications, object, name, perms, split_rule, &section_ids)
                .map_err(|kind| DeduplicationError { section_name: name, kind })?;
        }
    }

    Ok(deduplications)
}

fn deduplicate(
    deduplications: &mut BTreeMap<SectionId, Deduplication>,
    object: &mut Object,
    name: Interned<String>,
    perms: ElfPermissions,
    split_rule: SplitRule,
    section_ids: &[SectionId],
) -> Result<(), DeduplicationErrorKind> {
    let merged_id = object.sections.reserve_placeholder();
    let mut merged = Vec::new();
    let mut seen = BTreeMap::new();
    let mut sections_to_remove = Vec::new();
    let mut source = None;

    let merged_symbol_id = object.symbols.add(UpcomingSymbol::Section { section: merged_id })?;

    for &section_id in section_ids {
        let section = object.sections.get(section_id);
        let SectionContent::Data(part) = &section.content else {
            unreachable!("non-data section reached here");
        };

        match source {
            None => source = Some(section.source.clone()),
            Some(other_source) => source = Some(other_source.merge(&section.source)),
        }
        let mut deduplication = Deduplication {
            target: merged_id,
            target_symbol: merged_symbol_id,
            map: BTreeMap::new(),
        };
        for chunk in split(split_rule, &part.bytes) {
            let (chunk_start, chunk) = chunk?;
            match seen.get(&chunk) {
                Some(idx) => {
                    deduplication.map.insert(Length::from(chunk_start).as_offset()?, *idx);
                }
                None => {
                    let idx = merged.len();
                    merged.extend_from_slice(chunk);
                    seen.insert(chunk, Length::from(idx).as_offset()?);
                    deduplication.map.insert(
                        Length::from(chunk_start).as_offset()?,
                        Length::from(idx).as_offset()?,
                    );
                }
            }
        }
        deduplications.insert(section_id, deduplication);
        sections_to_remove.push(section_id);
    }

    object
        .sections
        .builder(
            &*name.resolve(),
            DataSection {
                perms,
                deduplication: match split_rule {
                    SplitRule::ZeroTerminatedString => ElfDeduplication::ZeroTerminatedStrings,
                    SplitRule::FixedSizeChunks { size } => {
                        ElfDeduplication::FixedSizeChunks { size }
                    }
                },
                bytes: merged,
                relocations: Vec::new(),
                inside_relro: false,
            },
        )
        .source(source.expect("no deduplicated sections"))
        .create_in_placeholder(merged_id);

    for id in sections_to_remove {
        object.sections.remove(id, None);
    }

    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum SplitRule {
    ZeroTerminatedString,
    FixedSizeChunks { size: NonZeroU64 },
}

fn split(
    rule: SplitRule,
    mut input: &[u8],
) -> impl Iterator<Item = Result<(usize, &[u8]), DeduplicationErrorKind>> {
    let initial_len = input.len();
    let mut current_pos = 0;
    std::iter::from_fn(move || {
        if input.is_empty() {
            None
        } else {
            let chunk = match rule {
                SplitRule::ZeroTerminatedString => match input.iter().position(|&b| b == 0) {
                    Some(cutoff) => {
                        let chunk = &input[..cutoff + 1];
                        input = &input[cutoff + 1..];
                        chunk
                    }
                    None => return Some(Err(DeduplicationErrorKind::NonZeroTerminatedString)),
                },
                SplitRule::FixedSizeChunks { size } => {
                    if (input.len() as u64) < size.get() {
                        return Some(Err(DeduplicationErrorKind::UnevenChunkSize {
                            len: initial_len as _,
                            chunks: size.get(),
                        }));
                    } else {
                        let chunk = &input[..size.get() as usize];
                        input = &input[size.get() as usize..];
                        chunk
                    }
                }
            };
            let chunk_start = current_pos;
            current_pos += chunk.len();
            Some(Ok((chunk_start, chunk)))
        }
    })
}

#[derive(Debug)]
pub(super) struct Deduplication {
    pub(super) target: SectionId,
    pub(super) target_symbol: SymbolId,
    pub(super) map: BTreeMap<Offset, Offset>,
}

#[derive(Debug, Display, Error)]
#[display("failed to deduplicate section {section_name}")]
pub(crate) struct DeduplicationError {
    section_name: Interned<String>,
    #[source]
    kind: DeduplicationErrorKind,
}

#[derive(Debug, Display, Error, PartialEq, Eq)]
pub(crate) enum DeduplicationErrorKind {
    #[display("sections with relocations are not supported")]
    RelocationsUnsupported,
    #[display("size of section is {len}, which cannot be divided in chunks of {chunks} bytes")]
    UnevenChunkSize { len: u64, chunks: u64 },
    #[display("there is a non-zero-terminated string in the content")]
    NonZeroTerminatedString,
    #[display("failed to create the symbol for the merged section")]
    SymbolCreation(#[from] LoadSymbolsError),
    #[display("the amount of data overflows the internal representation")]
    OutOfBounds(#[from] OutOfBoundsError),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_fixed_sized_chunks_ok() {
        assert_eq!(
            &[(0usize, &[1u8, 2, 3, 4] as &[u8]), (4, &[5, 6, 7, 8]), (8, &[9, 10, 11, 12])],
            split(
                SplitRule::FixedSizeChunks { size: NonZeroU64::new(4).unwrap() },
                &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]
            )
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
            .as_slice()
        )
    }

    #[test]
    fn test_split_fixed_chunks_uneven() {
        let mut split = split(
            SplitRule::FixedSizeChunks { size: NonZeroU64::new(4).unwrap() },
            &[1, 2, 3, 4, 5],
        );

        assert_eq!(Some(Ok((0, &[1u8, 2, 3, 4] as &[u8]))), split.next());
        assert_eq!(
            Some(Err(DeduplicationErrorKind::UnevenChunkSize { len: 5, chunks: 4 })),
            split.next()
        );
    }

    #[test]
    fn test_split_fixed_chunks_empty() {
        let mut split =
            split(SplitRule::FixedSizeChunks { size: NonZeroU64::new(4).unwrap() }, &[]);
        assert_eq!(None, split.next());
    }

    #[test]
    fn test_split_zero_terminated_ok() {
        assert_eq!(
            &[(0, &[1u8, 2, 3, 0] as &[u8]), (4, &[4, 5, 0]), (7, &[0]), (8, &[6, 0])],
            split(SplitRule::ZeroTerminatedString, &[1, 2, 3, 0, 4, 5, 0, 0, 6, 0])
                .collect::<Result<Vec<_>, _>>()
                .unwrap()
                .as_slice()
        );
    }

    #[test]
    fn test_split_zero_terminated_missing_terminator() {
        let mut split = split(SplitRule::ZeroTerminatedString, &[1, 2, 3, 4, 0, 5]);

        assert_eq!(Some(Ok((0, &[1u8, 2, 3, 4, 0] as &[u8]))), split.next());
        assert_eq!(Some(Err(DeduplicationErrorKind::NonZeroTerminatedString)), split.next());
    }

    #[test]
    fn test_split_zero_terminated_empty() {
        let mut split = split(SplitRule::ZeroTerminatedString, &[]);
        assert_eq!(None, split.next());
    }
}
