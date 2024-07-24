mod editor;

use crate::cli::Mode;
use crate::passes::generate_got::GOT;
use crate::passes::layout::{AddressResolutionError, Layout};
use crate::passes::relocate::editor::ByteEditor;
use crate::repr::object::Object;
use crate::repr::relocations::{Relocation, RelocationType};
use crate::repr::sections::{DataSection, SectionContent};
use crate::repr::symbols::{MissingGlobalSymbol, ResolveSymbolError, ResolvedSymbol, Symbols};
use crate::utils::ints::{Absolute, Address, Offset, OutOfBoundsError};
use plinky_elf::ids::serial::SectionId;
use plinky_elf::{ElfClass, ElfEnvironment};
use plinky_macros::{Display, Error};

pub(crate) fn run(object: &mut Object, layout: &Layout) -> Result<(), RelocationError> {
    let mut relocator = Relocator {
        layout,
        symbols: &mut object.symbols,
        dynamic_relocations: &mut object.dynamic_relocations,
        env: &object.env,
        got: object.got.as_ref(),
        mode: object.mode,
    };
    for section in object.sections.iter_mut() {
        match &mut section.content {
            SectionContent::Data(data) => {
                relocator.relocate(section.id, data)?;
            }
            _ => {}
        }
    }
    Ok(())
}

struct Relocator<'a> {
    env: &'a ElfEnvironment,
    got: Option<&'a GOT>,
    dynamic_relocations: &'a mut Vec<Relocation>,
    mode: Mode,
    layout: &'a Layout,
    symbols: &'a mut Symbols,
}

impl<'a> Relocator<'a> {
    fn relocate(
        &mut self,
        section_id: SectionId,
        data_section: &mut DataSection,
    ) -> Result<(), RelocationError> {
        for relocation in data_section.relocations.drain(..) {
            self.relocate_one(section_id, &relocation, &mut data_section.bytes).map_err(
                |inner| RelocationError {
                    section_id,
                    offset: relocation.offset,
                    relocation_type: relocation.type_,
                    inner,
                },
            )?;
        }
        Ok(())
    }

    fn relocate_one(
        &mut self,
        section_id: SectionId,
        relocation: &Relocation,
        bytes: &mut [u8],
    ) -> Result<(), RelocationErrorInner> {
        let mut editor = ByteEditor { relocation, bytes };
        match relocation.type_ {
            RelocationType::Absolute32 => {
                editor.write_u32(self.symbol_as_absolute(relocation, editor.addend_32()?)?)
            }
            RelocationType::AbsoluteSigned32 => {
                editor.write_i32(self.symbol_as_absolute(relocation, editor.addend_32()?)?)
            }
            RelocationType::Relative32 | RelocationType::PLT32 => {
                let symbol = self.symbol_as_address(relocation, editor.addend_32()?)?;
                let offset = self.layout.address(section_id, relocation.offset.into())?.1;
                editor.write_i32(symbol.as_offset()?.add(offset.as_offset()?.neg())?)
            }
            RelocationType::GOTRelative32 => {
                let got = self.got()?;
                let slot = got.offset(relocation.symbol);
                let section_addr = self.layout.address(section_id, relocation.offset.into())?.1;
                let got_addr = self.layout.address(got.id, 0.into())?.1;
                let addend = editor.addend_32()?;

                editor.write_i32(
                    got_addr
                        .as_offset()?
                        .add(slot)?
                        .add(addend)?
                        .add(section_addr.as_offset()?.neg())?,
                )
            }
            RelocationType::GOTIndex32 => {
                let slot = self.got()?.offset(relocation.symbol);
                let addend = editor.addend_32()?;
                editor.write_u32(slot.add(addend)?)
            }
            RelocationType::FillGOTSlot => match self.mode {
                Mode::PositionDependent => {
                    let symbol = self.symbol_as_absolute(relocation, 0.into())?;
                    match self.env.class {
                        ElfClass::Elf32 => editor.write_u32(symbol),
                        ElfClass::Elf64 => editor.write_u64(symbol),
                    }
                }
                Mode::PositionIndependent => {
                    self.symbols.get_mut(relocation.symbol).needed_by_dynamic = true;

                    self.dynamic_relocations.push(Relocation {
                        type_: RelocationType::FillGOTSlot,
                        symbol: relocation.symbol,
                        offset: self
                            .layout
                            .address(section_id, relocation.offset)?
                            .1
                            .as_offset()?,
                        addend: relocation.addend,
                    });

                    Ok(())
                }
            },
            RelocationType::GOTLocationRelative32 => {
                let got_addr = self.layout.address(self.got()?.id, 0.into())?.1;
                let addend = editor.addend_32()?;
                let offset = self.layout.address(section_id, relocation.offset.into())?.1;
                editor.write_i32(got_addr.as_offset()?.add(addend)?.add(offset.as_offset()?.neg())?)
            }
            RelocationType::OffsetFromGOT32 => {
                let symbol = self.symbol_as_address(relocation, editor.addend_32()?)?;
                let got = self.layout.address(self.got()?.id, 0.into())?.1;
                editor.write_i32(symbol.as_offset()?.add(got.as_offset()?.neg())?)
            }
        }
    }

    fn got(&self) -> Result<&GOT, RelocationErrorInner> {
        self.got.ok_or(RelocationErrorInner::GOTRelativeWithoutGOT)
    }

    fn symbol(
        &self,
        rel: &Relocation,
        offset: Offset,
    ) -> Result<ResolvedSymbol, RelocationErrorInner> {
        Ok(self.symbols.get(rel.symbol).resolve(self.layout, offset)?)
    }

    fn symbol_as_absolute(
        &self,
        rel: &Relocation,
        offset: Offset,
    ) -> Result<Absolute, RelocationErrorInner> {
        match self.symbol(rel, offset)? {
            ResolvedSymbol::Absolute(absolute) => Ok(absolute),
            ResolvedSymbol::Address { memory_address, .. } => Ok(memory_address.as_absolute()),
        }
    }

    fn symbol_as_address(
        &self,
        rel: &Relocation,
        offset: Offset,
    ) -> Result<Address, RelocationErrorInner> {
        match self.symbol(rel, offset)? {
            ResolvedSymbol::Absolute(_) => {
                return Err(RelocationErrorInner::RelativeRelocationWithAbsoluteValue);
            }
            ResolvedSymbol::Address { memory_address, .. } => Ok(memory_address),
        }
    }
}

#[derive(Debug, Error, Display)]
#[display(
    "failed to process relocation {relocation_type:?} in section {section_id:?} at offset {offset}"
)]
pub(crate) struct RelocationError {
    section_id: SectionId,
    offset: Offset,
    relocation_type: RelocationType,
    #[source]
    inner: RelocationErrorInner,
}

#[derive(Debug, Error, Display)]
pub(crate) enum RelocationErrorInner {
    #[transparent]
    MissingSymbol(MissingGlobalSymbol),
    #[transparent]
    SymbolResolution(ResolveSymbolError),
    #[transparent]
    AddressResolution(AddressResolutionError),
    #[transparent]
    OutOfBounds(OutOfBoundsError),
    #[display("relocation is trying to access offset {offset} (len: {len:#x}) on a section of size {size:#x}")]
    OutOfBoundsAccess { offset: Offset, len: usize, size: usize },
    #[display("relative relocations with absolute values are not supported")]
    RelativeRelocationWithAbsoluteValue,
    #[display("GOT-relative addressing used without a GOT")]
    GOTRelativeWithoutGOT,
}
