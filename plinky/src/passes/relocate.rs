use crate::passes::layout::{AddressResolutionError, Layout};
use crate::repr::object::Object;
use crate::repr::relocations::{Relocation, RelocationType};
use crate::repr::sections::{DataSection, SectionContent};
use crate::repr::symbols::{MissingGlobalSymbol, ResolveSymbolError, Symbols};
use plinky_elf::ids::serial::SectionId;
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
        relocation: &Relocation,
        bytes: &mut [u8],
    ) -> Result<(), RelocationError> {
        let mut editor = ByteEditor { relocation, bytes };
        match relocation.type_ {
            RelocationType::Absolute32 => {
                editor.write_32(self.symbol(relocation, editor.addend_32()?)?)
            }
            RelocationType::Relative32 | RelocationType::PLT32 => {
                let offset = self.layout.address(section_id, relocation.offset as i64)?.1 as i64;
                editor.write_32(self.symbol(relocation, editor.addend_32()?)? - offset)
            }
        }
    }

    fn symbol(&self, rel: &Relocation, offset: i64) -> Result<i64, RelocationError> {
        Ok(self.symbols.get(rel.symbol).resolve(self.layout, offset)?.as_u64() as i64)
    }
}

struct ByteEditor<'a> {
    relocation: &'a Relocation,
    bytes: &'a mut [u8],
}

impl ByteEditor<'_> {
    fn addend_32(&self) -> Result<i64, RelocationError> {
        match self.relocation.addend {
            Some(addend) => Ok(addend),
            None => Ok(i32::from_le_bytes(self.read()?).into()),
        }
    }

    fn write_32(&mut self, value: i64) -> Result<(), RelocationError> {
        self.write(
            &i32::try_from(value)
                .map_err(|_| RelocationError::RelocatedAddressTooLarge(value))?
                .to_le_bytes(),
        )
    }

    fn read<const LEN: usize>(&self) -> Result<[u8; LEN], RelocationError> {
        let err = Err(RelocationError::OutOfBoundsAccess {
            offset: self.relocation.offset,
            len: LEN,
            size: self.bytes.len(),
        });

        let Ok(start) = usize::try_from(self.relocation.offset) else { return err };
        let Some(end) = start.checked_add(LEN) else { return err };
        if end > self.bytes.len() {
            return err;
        }

        let mut data = [0; LEN];
        data.copy_from_slice(&self.bytes[start..end]);
        Ok(data)
    }

    fn write(&mut self, bytes: &[u8]) -> Result<(), RelocationError> {
        let err = Err(RelocationError::OutOfBoundsAccess {
            offset: self.relocation.offset,
            len: bytes.len(),
            size: self.bytes.len(),
        });

        let Ok(start) = usize::try_from(self.relocation.offset) else { return err };
        let Some(end) = start.checked_add(bytes.len()) else { return err };
        if end > self.bytes.len() {
            return err;
        }

        self.bytes[start..end].copy_from_slice(bytes);
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
    #[display("relocated address {f0:#x} is too large")]
    RelocatedAddressTooLarge(i64),
    #[display("relocation is trying to access offset {offset:#x} (len: {len:#x}) on a section of size {size:#x}")]
    OutOfBoundsAccess { offset: u64, len: usize, size: usize },
}
