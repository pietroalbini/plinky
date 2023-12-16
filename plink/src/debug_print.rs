use crate::cli::DebugPrint;
use crate::linker::{CallbackOutcome, LinkerCallbacks};
use crate::repr::object::{DataSectionPart, Object, SectionContent, SectionLayout};
use plink_elf::ids::serial::SerialIds;
use plink_elf::ElfObject;
use std::collections::BTreeMap;
use std::fmt::Debug;

pub(crate) struct DebugCallbacks {
    pub(crate) print: Option<DebugPrint>,
}

impl LinkerCallbacks for DebugCallbacks {
    fn on_inputs_loaded(&self, object: &Object<()>) -> CallbackOutcome {
        if let Some(DebugPrint::LoadedObject) = self.print {
            println!("{object:#x?}");
            CallbackOutcome::Stop
        } else {
            CallbackOutcome::Continue
        }
    }

    fn on_layout_calculated(&self, object: &Object<SectionLayout>) -> CallbackOutcome {
        if let Some(DebugPrint::Layout) = self.print {
            let addresses: BTreeMap<_, _> = object
                .sections
                .iter()
                .map(|(name, section)| {
                    let addresses: BTreeMap<_, _> = match &section.content {
                        SectionContent::Data(data) => data
                            .parts
                            .iter()
                            .map(|(id, part)| {
                                (
                                    id,
                                    match part {
                                        DataSectionPart::Real(real) => {
                                            DebugEither::A(real.layout.address)
                                        }
                                        DataSectionPart::DeduplicationFacade(_) => {
                                            DebugEither::B("<deduplication facade>")
                                        }
                                    },
                                )
                            })
                            .collect(),
                        SectionContent::Uninitialized(uninit) => uninit
                            .iter()
                            .map(|(id, part)| (id, DebugEither::A(part.layout.address)))
                            .collect(),
                    };
                    (name, addresses)
                })
                .collect();

            println!("Section addresses");
            println!("-----------------");
            println!("{addresses:#x?}");

            CallbackOutcome::Stop
        } else {
            CallbackOutcome::Continue
        }
    }

    fn on_relocations_applied(&self, object: &Object<SectionLayout>) -> CallbackOutcome {
        if let Some(DebugPrint::RelocatedObject) = self.print {
            println!("{object:#x?}");
            CallbackOutcome::Stop
        } else {
            CallbackOutcome::Continue
        }
    }

    fn on_elf_built(&self, elf: &ElfObject<SerialIds>) -> CallbackOutcome {
        if let Some(DebugPrint::FinalElf) = self.print {
            println!("{elf:#x?}");
            CallbackOutcome::Stop
        } else {
            CallbackOutcome::Continue
        }
    }
}

enum DebugEither<T: Debug, U: Debug> {
    A(T),
    B(U),
}

impl<T: Debug, U: Debug> Debug for DebugEither<T, U> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::A(v) => Debug::fmt(v, f),
            Self::B(v) => Debug::fmt(v, f),
        }
    }
}
