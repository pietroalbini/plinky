mod editor;

use crate::passes::generate_got::Got;
use crate::passes::generate_plt::Plt;
use crate::passes::relocate::editor::ByteEditor;
use crate::repr::object::Object;
use crate::repr::relocations::{Relocation, RelocationAddendError, RelocationType};
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
        got_plt: object.got_plt.as_ref(),
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
    got: Option<&'a Got>,
    got_plt: Option<&'a Got>,
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
        let mut editor = ByteEditor { endian: self.env.endian, relocation, bytes };
        match relocation.type_ {
            RelocationType::Absolute32 => {
                editor.write_u32(self.symbol_as_absolute(relocation, editor.addend()?)?)
            }
            RelocationType::Absolute64 => {
                editor.write_u64(self.symbol_as_absolute(relocation, editor.addend()?)?)
            }
            RelocationType::AbsoluteSigned32 => {
                editor.write_i32(self.symbol_as_absolute(relocation, editor.addend()?)?)
            }
            RelocationType::Relative32 => {
                let symbol = self.symbol_as_address(relocation, editor.addend()?)?;
                let offset = self.resolver.address(section_id, relocation.offset.into())?.1;
                editor.write_i32(symbol.as_offset()?.add(offset.as_offset()?.neg())?)
            }
            RelocationType::GOTRelative32 => {
                let got = self.got.expect("GOT was not generated with a GOT relocation");
                let slot = got.offset(relocation.symbol);
                let section_addr = self.resolver.address(section_id, relocation.offset.into())?.1;
                let got_addr = self.resolver.address(got.id, 0.into())?.1;
                let addend = editor.addend()?;

                editor.write_i32(
                    got_addr
                        .as_offset()?
                        .add(slot)?
                        .add(addend)?
                        .add(section_addr.as_offset()?.neg())?,
                )
            }
            RelocationType::PLT32 => {
                let plt = self.plt.expect("PLT was not generated with a PLT relocation");
                let plt_offset = *plt.offsets.get(&relocation.symbol).unwrap();
                let section_addr = self.resolver.address(section_id, relocation.offset.into())?.1;
                let plt_addr = self.resolver.address(plt.section, 0.into())?.1;
                let addend = editor.addend()?;

                editor.write_i32(
                    plt_addr
                        .as_offset()?
                        .add(plt_offset)?
                        .add(addend)?
                        .add(section_addr.as_offset()?.neg())?,
                )
            }
            RelocationType::GOTIndex32 => {
                let got = self.got.expect("GOT was not generated with a GOT relocation");
                let got_addr = self.resolver.address(got.id, 0.into())?.1;
                let slot = got.offset(relocation.symbol);
                let addend = editor.addend()?;

                // Here we are doing `.got - _GLOBAL_OFFSET_TABLE_ + slot`, as we need to figure
                // out the slot relative to the global offset table symbol.
                editor.write_i32(
                    got_addr
                        .as_offset()?
                        .add(self.got_symbol_addr()?.as_offset()?.neg())?
                        .add(slot)?
                        .add(addend)?,
                )
            }
            RelocationType::FillGotSlot | RelocationType::FillGotPltSlot => {
                let symbol = self.symbol_as_absolute(relocation, 0.into())?;
                match self.env.class {
                    ElfClass::Elf32 => editor.write_u32(symbol),
                    ElfClass::Elf64 => editor.write_u64(symbol),
                }
            }
            RelocationType::GOTLocationRelative32 => {
                let addend = editor.addend()?;
                let offset = self.resolver.address(section_id, relocation.offset.into())?.1;
                editor.write_i32(
                    self.got_symbol_addr()?
                        .as_offset()?
                        .add(addend)?
                        .add(offset.as_offset()?.neg())?,
                )
            }
            RelocationType::OffsetFromGOT32 => {
                let symbol = self.symbol_as_address(relocation, editor.addend()?)?;
                editor
                    .write_i32(symbol.as_offset()?.add(self.got_symbol_addr()?.as_offset()?.neg())?)
            }
        }
    }

    fn got_symbol_addr(&self) -> Result<Address, RelocationErrorInner> {
        Ok(self
            .resolver
            .address(
                self.got_plt
                    .expect("GOT.PLT not generated in a relocation requiring the GOT symbol")
                    .id,
                0.into(),
            )?
            .1)
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
            ResolvedSymbol::ExternallyDefined => {
                panic!("cannot do static reloc on external symbols")
            }
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
            ResolvedSymbol::ExternallyDefined => {
                panic!("cannot do static reloc on external symbols")
            }
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
    #[display(
        "relocation is trying to access offset {offset} (len: {len:#x}) on a section of size {size:#x}"
    )]
    OutOfBoundsAccess { offset: Offset, len: usize, size: usize },
    #[display("failed to obtain the relocation addend")]
    Addend(#[from] RelocationAddendError),
    #[display("relative relocations with absolute values are not supported")]
    RelativeRelocationWithAbsoluteValue,
}
