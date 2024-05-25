use crate::passes::relocate::RelocationErrorInner;
use crate::repr::relocations::Relocation;
use crate::utils::ints::{ExtractNumber, Offset, OutOfBoundsError};

pub(super) struct ByteEditor<'a> {
    pub(super) relocation: &'a Relocation,
    pub(super) bytes: &'a mut [u8],
}

impl ByteEditor<'_> {
    pub(super) fn addend_32(&self) -> Result<Offset, RelocationErrorInner> {
        match self.relocation.addend {
            Some(addend) => Ok(addend.into()),
            None => Ok(i32::from_le_bytes(self.read()?).into()),
        }
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

    fn read<const LEN: usize>(&self) -> Result<[u8; LEN], RelocationErrorInner> {
        let err = Err(RelocationErrorInner::OutOfBoundsAccess {
            offset: self.relocation.offset,
            len: LEN,
            size: self.bytes.len(),
        });

        let Ok(start) = usize::try_from(self.relocation.offset.extract()) else { return err };
        let Some(end) = start.checked_add(LEN) else { return err };
        if end > self.bytes.len() {
            return err;
        }

        let mut data = [0; LEN];
        data.copy_from_slice(&self.bytes[start..end]);
        Ok(data)
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
