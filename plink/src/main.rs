use crate::cli::DebugPrint;
use crate::linker::{link_driver, CallbackOutcome, LinkerCallbacks, LinkerError};
use crate::repr::object::{Object, SectionLayout, SectionMerge};
use plink_elf::ids::serial::SerialIds;
use std::collections::BTreeMap;
use std::error::Error;
use std::process::ExitCode;

mod cli;
mod linker;
mod passes;
mod repr;
mod interner;

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

    fn on_layout_calculated(
        &self,
        object: &Object<SectionLayout>,
        merges: &[SectionMerge],
    ) -> CallbackOutcome {
        if let Some(DebugPrint::Layout) = self.print {
            let addresses: BTreeMap<_, _> = object
                .sections
                .iter()
                .map(|(id, section)| (*id, section.layout.address))
                .collect();

            println!("Section addresses");
            println!("-----------------");
            println!("{addresses:#x?}");
            println!();
            println!("Section merges");
            println!("--------------");
            for merge in merges {
                println!("{merge:#x?}");
            }

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
