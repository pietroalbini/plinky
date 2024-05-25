use crate::passes::layout::{AddressResolutionError, Layout};
use crate::passes::generate_got::GOT;
use crate::repr::object::Object;
use crate::repr::relocations::{Relocation, RelocationType};
use crate::repr::sections::{DataSection, SectionContent};
use crate::repr::symbols::{MissingGlobalSymbol, ResolveSymbolError, ResolvedSymbol, Symbols};
use plinky_elf::ids::serial::SectionId;
use plinky_elf::{ElfClass, ElfEnvironment};
use plinky_macros::{Display, Error};

pub(crate) fn run(object: &mut Object, layout: &Layout) -> Result<(), RelocationError> {
    let relocator =
        Relocator { layout, symbols: &object.symbols, env: &object.env, got: object.got.as_ref() };
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
    env: &'a ElfEnvironment,
    got: Option<&'a GOT>,
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
                editor.write_u32(match self.symbol(relocation, editor.addend_32()?)? {
                    ResolvedSymbol::Absolute(absolute) => absolute.into(),
                    ResolvedSymbol::Address { memory_address, .. } => memory_address,
                })
            }
            RelocationType::AbsoluteSigned32 => {
                editor.write_i32(match self.symbol(relocation, editor.addend_32()?)? {
                    ResolvedSymbol::Absolute(absolute) => absolute.into(),
                    ResolvedSymbol::Address { memory_address, .. } => memory_address,
                })
            }
            RelocationType::Relative32 | RelocationType::PLT32 => {
                let symbol = match self.symbol(relocation, editor.addend_32()?)? {
                    ResolvedSymbol::Absolute(_) => {
                        return Err(RelocationError::RelativeRelocationWithAbsoluteValue);
                    }
                    ResolvedSymbol::Address { memory_address, .. } => memory_address,
                };
                let offset = self.layout.address(section_id, relocation.offset.into())?.1;
                editor.write_i32(
                    symbol
                        .checked_sub(offset)
                        .ok_or(RelocationError::RelocatedAddressOutOfBounds)?,
                )
            }
            RelocationType::GOTRelative32 => {
                let got = self.got()?;
                let slot: i128 = got.offset(relocation.symbol).into();
                let offset = self.layout.address(section_id, relocation.offset.into())?.1;
                let got_addr = self.layout.address(got.id, 0)?.1;
                let addend = editor.addend_32()?;

                editor.write_i32(
                    slot.checked_add(got_addr)
                        .and_then(|v| v.checked_add(addend))
                        .and_then(|v| v.checked_sub(offset))
                        .ok_or(RelocationError::RelocatedAddressOutOfBounds)?,
                )
            }
            RelocationType::GOTIndex32 => {
                let slot: i128 = self.got()?.offset(relocation.symbol).into();
                let addend = editor.addend_32()?;
                editor.write_u32(
                    slot.checked_add(addend).ok_or(RelocationError::RelocatedAddressOutOfBounds)?,
                )
            }
            RelocationType::FillGOTSlot => {
                let symbol = match self.symbol(relocation, 0)? {
                    ResolvedSymbol::Absolute(absolute) => absolute.into(),
                    ResolvedSymbol::Address { memory_address, .. } => memory_address,
                };
                match self.env.class {
                    ElfClass::Elf32 => editor.write_u32(symbol),
                    ElfClass::Elf64 => editor.write_u64(symbol),
                }
            }
            RelocationType::GOTLocationRelative32 => {
                let got_addr = self.layout.address(self.got()?.id, 0)?.1;
                let addend = editor.addend_32()?;
                let offset = self.layout.address(section_id, relocation.offset.into())?.1;
                editor.write_i32(
                    got_addr
                        .checked_add(addend)
                        .and_then(|v| v.checked_sub(offset))
                        .ok_or(RelocationError::RelocatedAddressOutOfBounds)?,
                )
            }
            RelocationType::OffsetFromGOT32 => {
                let symbol = match self.symbol(relocation, editor.addend_32()?)? {
                    ResolvedSymbol::Absolute(_) => {
                        return Err(RelocationError::RelativeRelocationWithAbsoluteValue);
                    }
                    ResolvedSymbol::Address { memory_address, .. } => memory_address,
                };
                let got = self.layout.address(self.got()?.id, 0)?.1;
                editor.write_i32(
                    symbol.checked_sub(got).ok_or(RelocationError::RelocatedAddressOutOfBounds)?,
                )
            }
        }
    }

    fn got(&self) -> Result<&GOT, RelocationError> {
        self.got.ok_or(RelocationError::GOTRelativeWithoutGOT)
    }

    fn symbol(&self, rel: &Relocation, offset: i128) -> Result<ResolvedSymbol, RelocationError> {
        Ok(self.symbols.get(rel.symbol).resolve(self.layout, offset)?)
    }
}

struct ByteEditor<'a> {
    relocation: &'a Relocation,
    bytes: &'a mut [u8],
}

impl ByteEditor<'_> {
    fn addend_32(&self) -> Result<i128, RelocationError> {
        match self.relocation.addend {
            Some(addend) => Ok(addend.into()),
            None => Ok(i32::from_le_bytes(self.read()?).into()),
        }
    }

    fn write_u32(&mut self, value: i128) -> Result<(), RelocationError> {
        self.write(
            &u32::try_from(value)
                .map_err(|_| RelocationError::RelocatedAddressOutOfBounds)?
                .to_le_bytes(),
        )
    }

    fn write_u64(&mut self, value: i128) -> Result<(), RelocationError> {
        self.write(
            &u64::try_from(value)
                .map_err(|_| RelocationError::RelocatedAddressOutOfBounds)?
                .to_le_bytes(),
        )
    }

    fn write_i32(&mut self, value: i128) -> Result<(), RelocationError> {
        self.write(
            &i32::try_from(value)
                .map_err(|_| RelocationError::RelocatedAddressOutOfBounds)?
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
    #[display("relocated address is out of bounds")]
    RelocatedAddressOutOfBounds,
    #[display("relocation is trying to access offset {offset:#x} (len: {len:#x}) on a section of size {size:#x}")]
    OutOfBoundsAccess { offset: u64, len: usize, size: usize },
    #[display("relative relocations with absolute values are not supported")]
    RelativeRelocationWithAbsoluteValue,
    #[display("GOT-relative addressing used without a GOT")]
    GOTRelativeWithoutGOT,
}
