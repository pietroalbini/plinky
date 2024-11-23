mod editor;

use crate::passes::generate_got::GOT;
use crate::passes::generate_plt::Plt;
use crate::passes::relocate::editor::ByteEditor;
use crate::repr::object::Object;
use crate::repr::relocations::{Relocation, RelocationType};
use crate::repr::sections::{DataSection, SectionContent, SectionId};
use crate::repr::symbols::{MissingGlobalSymbol, ResolveSymbolError, ResolvedSymbol, Symbols};
use crate::utils::address_resolver::{AddressResolutionError, AddressResolver};
use plinky_elf::{ElfClass, ElfEnvironment};
use plinky_macros::{Display, Error};
use plinky_utils::ints::{Absolute, Address, Offset, OutOfBoundsError};

pub(crate) fn run(
    object: &mut Object,
    resolver: &AddressResolver<'_>,
) -> Result<(), RelocationError> {
    let mut relocator = Relocator {
        symbols: &mut object.symbols,
        env: &object.env,
        got: object.got.as_ref(),
        plt: object.plt.as_ref(),
        resolver,
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
    plt: Option<&'a Plt>,
    symbols: &'a mut Symbols,
    resolver: &'a AddressResolver<'a>,
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
            RelocationType::Relative32 => {
                let symbol = self.symbol_as_address(relocation, editor.addend_32()?)?;
                let offset = self.resolver.address(section_id, relocation.offset.into())?.1;
                editor.write_i32(symbol.as_offset()?.add(offset.as_offset()?.neg())?)
            }
            RelocationType::GOTRelative32 => {
                let got = self.got()?;
                let slot = got.offset(relocation.symbol);
                let section_addr = self.resolver.address(section_id, relocation.offset.into())?.1;
                let got_addr = self.resolver.address(got.id, 0.into())?.1;
                let addend = editor.addend_32()?;

                editor.write_i32(
                    got_addr
                        .as_offset()?
                        .add(slot)?
                        .add(addend)?
                        .add(section_addr.as_offset()?.neg())?,
                )
            }
            RelocationType::PLT32 => {
                let plt = self.plt.ok_or(RelocationErrorInner::PltWithoutPlt)?;
                let plt_offset = *plt.offsets.get(&relocation.symbol).unwrap();
                let section_addr = self.resolver.address(section_id, relocation.offset.into())?.1;
                let plt_addr = self.resolver.address(plt.section, 0.into())?.1;
                let addend = editor.addend_32()?;

                editor.write_i32(
                    plt_addr
                        .as_offset()?
                        .add(plt_offset)?
                        .add(addend)?
                        .add(section_addr.as_offset()?.neg())?,
                )
            }
            RelocationType::GOTIndex32 => {
                let slot = self.got()?.offset(relocation.symbol);
                let addend = editor.addend_32()?;
                editor.write_u32(slot.add(addend)?)
            }
            RelocationType::FillGotSlot | RelocationType::FillGotPltSlot => {
                let symbol = self.symbol_as_absolute(relocation, 0.into())?;
                match self.env.class {
                    ElfClass::Elf32 => editor.write_u32(symbol),
                    ElfClass::Elf64 => editor.write_u64(symbol),
                }
            }
            RelocationType::GOTLocationRelative32 => {
                let got_addr = self.resolver.address(self.got()?.id, 0.into())?.1;
                let addend = editor.addend_32()?;
                let offset = self.resolver.address(section_id, relocation.offset.into())?.1;
                editor.write_i32(got_addr.as_offset()?.add(addend)?.add(offset.as_offset()?.neg())?)
            }
            RelocationType::OffsetFromGOT32 => {
                let symbol = self.symbol_as_address(relocation, editor.addend_32()?)?;
                let got_plt = self.resolver.address(self.got()?.id, 0.into())?.1;
                editor.write_i32(symbol.as_offset()?.add(got_plt.as_offset()?.neg())?)
            }
        }
    }

    fn got(&self) -> Result<&GOT, RelocationErrorInner> {
        self.got.ok_or(RelocationErrorInner::GotRelativeWithoutGot)
    }

    fn symbol(
        &self,
        rel: &Relocation,
        offset: Offset,
    ) -> Result<ResolvedSymbol, RelocationErrorInner> {
        Ok(self.symbols.get(rel.symbol).resolve(self.resolver, offset)?)
    }

    fn symbol_as_absolute(
        &self,
        rel: &Relocation,
        offset: Offset,
    ) -> Result<Absolute, RelocationErrorInner> {
        match self.symbol(rel, offset)? {
            ResolvedSymbol::ExternallyDefined => todo!(), // TODO: implement this
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
            ResolvedSymbol::ExternallyDefined => todo!(), // TODO: implement this
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
    GotRelativeWithoutGot,
    #[display("PLT relocation used without a PLT")]
    PltWithoutPlt,
}
