use crate::interner::Interned;
use crate::repr::object::{
    DataSection, DataSectionPart, DataSectionPartReal, DeduplicationFacade, Object, SectionContent,
};
use plinky_elf::ids::serial::{SectionId, SerialIds};
use plinky_elf::{ElfDeduplication, RawBytes};
use plinky_macros::{Display, Error};
use std::collections::BTreeMap;
use std::num::NonZeroU64;

pub(crate) fn run(object: &mut Object<()>, ids: &mut SerialIds) -> Result<(), DeduplicationError> {
    for (&section_name, section) in &mut object.sections {
        let SectionContent::Data(data) = &mut section.content else {
            continue;
        };
        let split_rule = match data.deduplication {
            ElfDeduplication::Disabled => continue,
            ElfDeduplication::ZeroTerminatedStrings => SplitRule::ZeroTerminatedString,
            ElfDeduplication::FixedSizeChunks { size } => SplitRule::FixedSizeChunks { size },
        };

        // Not sure exactly whether relocations inside of deduplicatable sections are ever used in
        // the wild, so for now let's error out if we encounter this.
        for part in data.parts.values() {
            match part {
                DataSectionPart::Real(real) => {
                    if !real.relocations.is_empty() {
                        return Err(DeduplicationError {
                            section_name,
                            kind: DeduplicationErrorKind::RelocationsUnsupported,
                        });
                    }
                }
                DataSectionPart::DeduplicationFacade(_) => {
                    unreachable!("deduplication facades should not be present at this stage")
                }
            }
        }

        deduplicate(ids, &mut object.section_ids_to_names, section_name, split_rule, data)
            .map_err(|kind| DeduplicationError { section_name, kind })?;
    }

    Ok(())
}

fn deduplicate(
    ids: &mut SerialIds,
    section_ids_to_names: &mut BTreeMap<SectionId, Interned<String>>,
    section_name: Interned<String>,
    split_rule: SplitRule,
    data: &mut DataSection<()>,
) -> Result<(), DeduplicationErrorKind> {
    let merged_id = ids.allocate_section_id();
    let mut merged = Vec::new();
    let mut seen = BTreeMap::new();
    let mut facades_to_replace = BTreeMap::new();
    let mut source = None;

    for (&section_id, part) in data.parts.iter() {
        let (bytes, facade_source) = match part {
            DataSectionPart::Real(real) => {
                match source {
                    None => source = Some(real.source.clone()),
                    Some(other_source) => source = Some(other_source.merge(&real.source)),
                }
                (&real.bytes.0, real.source.clone())
            }
            DataSectionPart::DeduplicationFacade(_) => {
                unreachable!("deduplication facades should not be present at this stage")
            }
        };
        let mut facade = DeduplicationFacade {
            section_id: merged_id,
            source: facade_source,
            offset_map: BTreeMap::new(),
        };
        for chunk in split(split_rule, bytes) {
            let (chunk_start, chunk) = chunk?;
            match seen.get(&chunk) {
                Some(idx) => {
                    facade.offset_map.insert(chunk_start as _, *idx as _);
                }
                None => {
                    let idx = merged.len();
                    merged.extend_from_slice(chunk);
                    seen.insert(chunk, idx);
                    facade.offset_map.insert(chunk_start as _, idx as _);
                }
            }
        }
        facades_to_replace.insert(section_id, facade);
    }

    data.parts.insert(
        merged_id,
        DataSectionPart::Real(DataSectionPartReal {
            source: source.expect("no deduplicated sections"),
            bytes: RawBytes(merged),
            relocations: Vec::new(),
            layout: (),
        }),
    );
    section_ids_to_names.insert(merged_id, section_name);

    for (section_id, part) in data.parts.iter_mut() {
        if let Some(facade) = facades_to_replace.remove(section_id) {
            *part = DataSectionPart::DeduplicationFacade(facade);
        }
    }

    Ok(())
}

#[derive(Clone, Copy)]
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
