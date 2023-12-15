use crate::cli::DebugPrint;
use crate::linker::{link_driver, CallbackOutcome, LinkerCallbacks, LinkerError};
use crate::repr::object::{Object, SectionContent, SectionLayout};
use plink_elf::ids::serial::SerialIds;
use std::collections::BTreeMap;
use std::error::Error;
use std::process::ExitCode;

mod cli;
mod interner;
mod linker;
mod passes;
mod repr;

fn app() -> Result<(), Box<dyn Error>> {
    let options = cli::parse(std::env::args().skip(1))?;

    let callbacks = DebugCallbacks {
        print: options.debug_print,
    };
    match link_driver(&options, &callbacks) {
        Ok(()) => {}
        Err(LinkerError::CallbackEarlyExit) => {}
        Err(err) => return Err(err.into()),
    }

    Ok(())
}

struct DebugCallbacks {
    print: Option<DebugPrint>,
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
                            .map(|(id, part)| (id, part.layout.address))
                            .collect(),
                        SectionContent::Uninitialized(uninit) => uninit
                            .iter()
                            .map(|(id, part)| (id, part.layout.address))
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

    fn on_elf_built(&self, elf: &plink_elf::ElfObject<SerialIds>) -> CallbackOutcome {
        if let Some(DebugPrint::FinalElf) = self.print {
            println!("{elf:#x?}");
            CallbackOutcome::Stop
        } else {
            CallbackOutcome::Continue
        }
    }
}

fn main() -> ExitCode {
    match app() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("error: {err}");

            let mut source = err.source();
            while let Some(s) = source {
                eprintln!("caused by: {s}");
                source = s.source();
            }

            ExitCode::FAILURE
        }
    }
}
