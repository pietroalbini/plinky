use crate::cli::DebugPrint;
use crate::linker::{CallbackOutcome, LinkerCallbacks};
use crate::repr::object::{Object, SectionContent, SectionLayout};
use plink_elf::ids::serial::SerialIds;
use plink_elf::ElfObject;
use std::collections::BTreeMap;

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
                        SectionContent::Data(data) => {
                            data.parts.iter().map(|(id, part)| (id, part.layout.address)).collect()
                        }
                        SectionContent::Uninitialized(uninit) => {
                            uninit.iter().map(|(id, part)| (id, part.layout.address)).collect()
                        }
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
