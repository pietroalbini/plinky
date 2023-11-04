use crate::linker::layout::SectionLayout;
use crate::linker::object::{GetSymbolAddressError, Object};
use plink_elf::ids::serial::SerialIds;
use plink_elf::{ElfEnvironment, ElfObject, ElfType};
use std::collections::BTreeMap;
use std::num::NonZeroU64;

pub(super) struct ElfBuilderContext {
    pub(super) entrypoint: String,
    pub(super) env: ElfEnvironment,
    pub(super) object: Object<SectionLayout>,
}

pub(super) struct ElfBuilder {
    ctx: ElfBuilderContext,
}

impl ElfBuilder {
    pub(super) fn new(ctx: ElfBuilderContext) -> Self {
        Self { ctx }
    }

    pub(super) fn build(self) -> Result<ElfObject<SerialIds>, ElfBuilderError> {
        let entry = self.prepare_entry_point()?;

        Ok(ElfObject {
            env: self.ctx.env,
            type_: ElfType::Executable,
            entry,
            flags: 0,
            sections: BTreeMap::new(),
            segments: Vec::new(),
        })
    }

    fn prepare_entry_point(&self) -> Result<Option<NonZeroU64>, ElfBuilderError> {
        Ok(Some(
            NonZeroU64::new(
                self.ctx
                    .object
                    .global_symbol_address(&self.ctx.entrypoint)
                    .map_err(ElfBuilderError::InvalidEntrypoint)?,
            )
            .ok_or_else(|| ElfBuilderError::EntrypointIsZero(self.ctx.entrypoint.clone()))?,
        ))
    }
}

#[derive(Debug)]
pub(crate) enum ElfBuilderError {
    InvalidEntrypoint(GetSymbolAddressError),
    EntrypointIsZero(String),
}

impl std::error::Error for ElfBuilderError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ElfBuilderError::InvalidEntrypoint(err) => Some(err),
            ElfBuilderError::EntrypointIsZero(_) => None,
        }
    }
}

impl std::fmt::Display for ElfBuilderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ElfBuilderError::InvalidEntrypoint(_) => {
                f.write_str("failed to find the entry point of the executable")
            }
            ElfBuilderError::EntrypointIsZero(entrypoint) => {
                write!(f, "entry point symbol {entrypoint} is zero")
            }
        }
    }
}
