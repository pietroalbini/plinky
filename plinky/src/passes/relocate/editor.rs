use crate::passes::relocate::RelocationErrorInner;
use crate::repr::relocations::Relocation;
use plinky_utils::ints::{ExtractNumber, Offset, OutOfBoundsError};
use plinky_elf::ElfEndian;

pub(super) struct ByteEditor<'a> {
    pub(super) endian: ElfEndian,
    pub(super) relocation: &'a Relocation,
    pub(super) bytes: &'a mut [u8],
}

impl ByteEditor<'_> {
    pub(super) fn addend(&self) -> Result<Offset, RelocationErrorInner> {
        Ok(self.relocation.addend(self.endian, &self.bytes)?)
    }

    pub(super) fn write_u32<N>(&mut self, value: N) -> Result<(), RelocationErrorInner>
    where
        N: ExtractNumber,
        N::Type: TryInto<u32>,
    {
        self.write(&value.extract().try_into().map_err(|_| OutOfBoundsError)?.to_le_bytes())
    }

    pub(super) fn write_u64<N>(&mut self, value: N) -> Result<(), RelocationErrorInner>
    where
        N: ExtractNumber,
        N::Type: TryInto<u64>,
    {
        self.write(&value.extract().try_into().map_err(|_| OutOfBoundsError)?.to_le_bytes())
    }

    pub(super) fn write_i32<N>(&mut self, value: N) -> Result<(), RelocationErrorInner>
    where
        N: ExtractNumber,
        N::Type: TryInto<i32>,
    {
        self.write(&value.extract().try_into().map_err(|_| OutOfBoundsError)?.to_le_bytes())
    }

    fn write(&mut self, bytes: &[u8]) -> Result<(), RelocationErrorInner> {
        let err = Err(RelocationErrorInner::OutOfBoundsAccess {
            offset: self.relocation.offset,
            len: bytes.len(),
            size: self.bytes.len(),
        });

        let Ok(start) = usize::try_from(self.relocation.offset.extract()) else { return err };
        let Some(end) = start.checked_add(bytes.len()) else { return err };
        if end > self.bytes.len() {
            return err;
        }

        self.bytes[start..end].copy_from_slice(bytes);
        Ok(())
    }
}
