use crate::passes::layout::{AddressResolutionError, Layout};
use crate::repr::object::Object;
use crate::repr::sections::{DataSection, SectionContent};
use crate::repr::symbols::{MissingGlobalSymbol, ResolveSymbolError, Symbols};
use plinky_elf::ids::serial::{SectionId, SerialIds};
use plinky_elf::{ElfRelocation, ElfRelocationType};
use plinky_macros::{Display, Error};

pub(crate) fn run(object: &mut Object, layout: &Layout) -> Result<(), RelocationError> {
    let relocator = Relocator { layout, symbols: &object.symbols };
    for section in object.sections.iter_mut() {
        match &mut section.content {
            SectionContent::Data(data) => {
                relocator.relocate(section.id, data)?;
            }
            SectionContent::Uninitialized(_) => {}
        }
    }
    Ok(())
}

struct Relocator<'a> {
    layout: &'a Layout,
    symbols: &'a Symbols,
}

impl<'a> Relocator<'a> {
    fn relocate(
        &self,
        section_id: SectionId,
        data_section: &mut DataSection,
    ) -> Result<(), RelocationError> {
        for relocation in data_section.relocations.drain(..) {
            self.relocate_one(section_id, &relocation, &mut data_section.bytes)?;
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
                let offset = self.layout.address(section_id, relocation.offset as i64)? as i64;
                editor.write_32(self.symbol(relocation, editor.addend_32())? - offset)
            }
            other => Err(RelocationError::UnsupportedRelocation(other)),
        }
    }

    fn symbol(&self, rel: &ElfRelocation<SerialIds>, offset: i64) -> Result<i64, RelocationError> {
        Ok(self.symbols.get(rel.symbol).resolve(self.layout, offset)?.as_u64() as i64)
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
    #[transparent]
    SymbolResolution(ResolveSymbolError),
    #[transparent]
    AddressResolution(AddressResolutionError),
    #[display("unsupported relocation type {f0:?}")]
    UnsupportedRelocation(ElfRelocationType),
    #[display("relocated address {f0:#x} is too large")]
    RelocatedAddressTooLarge(i64),
}
