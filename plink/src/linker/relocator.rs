use crate::linker::layout::SectionLayout;
use crate::linker::object::ProgramSection;
use crate::linker::symbols::{MissingGlobalSymbol, Symbols};
use plink_elf::ids::serial::{SectionId, SerialIds, SymbolId};
use plink_elf::{ElfRelocation, ElfRelocationType, ElfSymbolDefinition};
use plink_macros::Error;
use std::collections::BTreeMap;

pub(super) struct Relocator<'a> {
    section_addresses: BTreeMap<SectionId, u64>,
    symbols: &'a Symbols,
}

impl<'a> Relocator<'a> {
    pub(super) fn new(
        sections: &BTreeMap<SectionId, ProgramSection<SectionLayout>>,
        symbols: &'a Symbols,
    ) -> Self {
        Self {
            section_addresses: sections
                .iter()
                .map(|(id, section)| (*id, section.layout.address))
                .collect(),
            symbols,
        }
    }

    pub(super) fn relocate(
        &self,
        section: &mut ProgramSection<SectionLayout>,
    ) -> Result<(), RelocationError> {
        for relocation in section.relocations.drain(..) {
            self.relocate_one(&relocation, &mut section.program.raw.0)?;
        }
        Ok(())
    }

    fn relocate_one(
        &self,
        relocation: &ElfRelocation<SerialIds>,
        bytes: &mut [u8],
    ) -> Result<(), RelocationError> {
        let mut editor = ByteEditor { relocation, bytes };
        match relocation.relocation_type {
            ElfRelocationType::X86_64_32 => self.x86_64_32(&relocation, &mut editor),
            ElfRelocationType::X86_64_PC32 => self.x86_64_pc32(&relocation, &mut editor),
            other => Err(RelocationError::UnsupportedRelocation(other)),
        }
    }

    fn x86_64_32(
        &self,
        rel: &ElfRelocation<SerialIds>,
        editor: &mut ByteEditor<'_>,
    ) -> Result<(), RelocationError> {
        editor.write_u32(self.symbol(rel)? + editor.addend_32())
    }

    fn x86_64_pc32(
        &self,
        rel: &ElfRelocation<SerialIds>,
        editor: &mut ByteEditor<'_>,
    ) -> Result<(), RelocationError> {
        editor.write_u32(self.symbol(rel)? + editor.addend_32() - rel.offset as i64)
    }

    fn symbol(&self, rel: &ElfRelocation<SerialIds>) -> Result<i64, RelocationError> {
        let symbol = self.symbols.get(rel.symbol)?;
        match symbol.definition {
            ElfSymbolDefinition::Undefined => Err(RelocationError::UndefinedSymbol(rel.symbol)),
            ElfSymbolDefinition::Absolute => Ok(symbol.value as i64),
            ElfSymbolDefinition::Common => todo!(),
            ElfSymbolDefinition::Section(section) => {
                let section_addr = self
                    .section_addresses
                    .get(&section)
                    .expect("inconsistent section id");
                Ok((*section_addr + symbol.value) as i64)
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

    fn write_u32(&mut self, value: i64) -> Result<(), RelocationError> {
        let bytes = u32::try_from(value)
            .map_err(|_| RelocationError::RelocatedAddressTooLarge(value))?
            .to_le_bytes();

        let offset = self.relocation.offset as usize;
        self.bytes[offset..offset + 4].copy_from_slice(&bytes);

        Ok(())
    }
}

#[derive(Debug, Error)]
pub(crate) enum RelocationError {
    MissingSymbol(#[from] MissingGlobalSymbol),
    UndefinedSymbol(SymbolId),
    UnsupportedRelocation(ElfRelocationType),
    RelocatedAddressTooLarge(i64),
}

impl std::fmt::Display for RelocationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RelocationError::MissingSymbol(_) => {
                f.write_str("missing symbol found during relocation")
            }
            RelocationError::UndefinedSymbol(symbol) => write!(f, "undefined symbol {symbol:?}"),
            RelocationError::UnsupportedRelocation(type_) => {
                write!(f, "unsupported relocation type {type_:?}")
            }
            RelocationError::RelocatedAddressTooLarge(addr) => {
                write!(f, "relocated address {addr:#x} is too large")
            }
        }
    }
}
