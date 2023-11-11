mod cursor;
mod header;
mod replacements;

use crate::ids::{ElfIds, StringIdGetters};
use crate::utils::WriteSeek;
use crate::writer::cursor::WriteCursor;
use crate::writer::replacements::Replacements;
use crate::ElfObject;
use crate::errors::WriteError;

pub(crate) struct Writer<'a, I>
where
    I: ElfIds,
    I::StringId: StringIdGetters<I>,
{
    cursor: WriteCursor<'a>,
    object: &'a ElfObject<I>,
    replacements: Replacements,
}

impl<'a, I> Writer<'a, I>
where
    I: ElfIds,
    I::StringId: StringIdGetters<I>,
{
    pub(crate) fn new(write_to: &'a mut dyn WriteSeek, object: &'a ElfObject<I>) -> Self {
        Self {
            cursor: WriteCursor::new(write_to, &object),
            object,
            replacements: Replacements::new(),
        }
    }

    pub(crate) fn write(mut self) -> Result<(), WriteError> {
        self.write_header()?;
        Ok(())
    }
}
