use crate::repr::object::{
    DataSectionPart, DataSectionPartReal, DeduplicationFacade, Object, SectionContent,
    SectionLayout,
};
use crate::repr::symbols::{MissingGlobalSymbol, Symbols};
use plinky_elf::ids::serial::{SectionId, SerialIds, SymbolId};
use plinky_elf::{ElfRelocation, ElfRelocationType, ElfSymbolDefinition};
use plinky_macros::{Display, Error};
use std::collections::BTreeMap;

pub(crate) fn run(object: &mut Object<SectionLayout>) -> Result<(), RelocationError> {
    let relocator =
        Relocator { section_resolvers: fetch_section_resolvers(object), symbols: &object.symbols };
    for section in object.sections.values_mut() {
        match &mut section.content {
            SectionContent::Data(data) => {
                for (&id, part) in &mut data.parts {
                    match part {
                        DataSectionPart::Real(real) => relocator.relocate(id, real)?,
                        DataSectionPart::DeduplicationFacade(_) => {}
                    }
                }
            }
            SectionContent::Uninitialized(_) => {}
        }
    }
    Ok(())
}

fn fetch_section_resolvers(object: &Object<SectionLayout>) -> BTreeMap<SectionId, AddressResolver> {
    object
        .sections
        .values()
        .flat_map(|section| -> Box<dyn Iterator<Item = _>> {
            match &section.content {
                SectionContent::Data(data) => Box::new(data.parts.iter().map(|(&id, part)| {
                    (
                        id,
                        match part {
                            DataSectionPart::Real(real) => {
                                AddressResolver::Section { address: real.layout.address }
                            }
                            DataSectionPart::DeduplicationFacade(facade) => {
                                AddressResolver::DeduplicationFacade(facade.clone())
                            }
                        },
                    )
                })),
                SectionContent::Uninitialized(uninit) => {
                    Box::new(uninit.iter().map(|(&id, part)| {
                        (id, AddressResolver::Section { address: part.layout.address })
                    }))
                }
            }
        })
        .collect()
}

struct Relocator<'a> {
    section_resolvers: BTreeMap<SectionId, AddressResolver>,
    symbols: &'a Symbols,
}

impl<'a> Relocator<'a> {
    fn relocate(
        &self,
        section_id: SectionId,
        data_section: &mut DataSectionPartReal<SectionLayout>,
    ) -> Result<(), RelocationError> {
        for relocation in data_section.relocations.drain(..) {
            self.relocate_one(section_id, &relocation, &mut data_section.bytes.0)?;
        }
        Ok(())
    }

    fn relocate_one(
        &self,
        section_id: SectionId,
        relocation: &ElfRelocation<SerialIds>,
        bytes: &mut [u8],
    ) -> Result<(), RelocationError> {
        let mut editor = ByteEditor { relocation, bytes };
        match relocation.relocation_type {
            ElfRelocationType::X86_64_32
            | ElfRelocationType::X86_64_32S
            | ElfRelocationType::X86_32 => {
                editor.write_32(self.symbol(relocation, editor.addend_32())?)
            }
            ElfRelocationType::X86_64_PC32
            | ElfRelocationType::X86_PC32
            | ElfRelocationType::X86_64_PLT32 => {
                let offset = self.resolve(section_id, relocation.offset as i64)?;
                editor.write_32(self.symbol(relocation, editor.addend_32())? - offset)
            }
            other => Err(RelocationError::UnsupportedRelocation(other)),
        }
    }

    fn symbol(&self, rel: &ElfRelocation<SerialIds>, offset: i64) -> Result<i64, RelocationError> {
        let symbol = self.symbols.get(rel.symbol)?;
        match symbol.definition {
            ElfSymbolDefinition::Undefined => Err(RelocationError::UndefinedSymbol(rel.symbol)),
            ElfSymbolDefinition::Absolute => Ok(symbol.value as i64 + offset),
            ElfSymbolDefinition::Common => todo!(),
            ElfSymbolDefinition::Section(section) => {
                Ok(self.resolve(section, symbol.value as i64 + offset)?)
            }
        }
    }

    fn resolve(&self, section: SectionId, offset: i64) -> Result<i64, RelocationError> {
        let resolver = self.section_resolvers.get(&section).expect("inconsistent section id");
        resolver.resolve(self, offset)
    }
}

enum AddressResolver {
    Section { address: u64 },
    DeduplicationFacade(DeduplicationFacade),
}

impl AddressResolver {
    fn resolve(&self, relocator: &Relocator<'_>, offset: i64) -> Result<i64, RelocationError> {
        match self {
            AddressResolver::Section { address } => Ok(*address as i64 + offset),
            AddressResolver::DeduplicationFacade(facade) => {
                let base = match relocator.section_resolvers.get(&facade.section_id) {
                    Some(AddressResolver::Section { address }) => *address,
                    Some(AddressResolver::DeduplicationFacade(_)) => {
                        return Err(RelocationError::RecursiveDuplicationFacadesNotAllowed);
                    }
                    None => panic!("facade points to missing section"),
                };
                let map_key = u64::try_from(offset)
                    .map_err(|_| RelocationError::NegativeOffsetToAccessDeduplications)?;
                match facade.offset_map.get(&map_key) {
                    Some(&mapped) => Ok(base as i64 + mapped as i64),
                    None => {
                        Err(RelocationError::UnsupportedUnalignedReferenceInDeduplicatedSections)
                    }
                }
            }
        }
    }
}

struct ByteEditor<'a> {
    relocation: &'a ElfRelocation<SerialIds>,
    bytes: &'a mut [u8],
}

impl ByteEditor<'_> {
    fn addend_32(&self) -> i64 {
        match self.relocation.addend {
            Some(addend) => addend,
            None => {
                let offset = self.relocation.offset as usize;
                let bytes = &self.bytes[offset..offset + 4];
                i32::from_le_bytes(bytes.try_into().unwrap()).into()
            }
        }
    }

    fn write_32(&mut self, value: i64) -> Result<(), RelocationError> {
        let bytes = i32::try_from(value)
            .map_err(|_| RelocationError::RelocatedAddressTooLarge(value))?
            .to_le_bytes();

        let offset = self.relocation.offset as usize;
        self.bytes[offset..offset + 4].copy_from_slice(&bytes);

        Ok(())
    }
}

#[derive(Debug, Error, Display)]
pub(crate) enum RelocationError {
    #[transparent]
    MissingSymbol(MissingGlobalSymbol),
    #[display("undefined symbol {f0:?}")]
    UndefinedSymbol(SymbolId),
    #[display("unsupported relocation type {f0:?}")]
    UnsupportedRelocation(ElfRelocationType),
    #[display("relocated address {f0:#x} is too large")]
    RelocatedAddressTooLarge(i64),
    #[display("recursive relocation facades are not allowed")]
    RecursiveDuplicationFacadesNotAllowed,
    #[display("unsupported unaligned reference in deduplicated sections")]
    UnsupportedUnalignedReferenceInDeduplicatedSections,
    #[display("a negative offset was used to access deduplications")]
    NegativeOffsetToAccessDeduplications,
}
