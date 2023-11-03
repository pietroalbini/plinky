use crate::cli::DebugPrint;
use crate::linker::Linker;
use std::error::Error;
use std::process::ExitCode;

mod cli;
mod linker;

fn app() -> Result<(), Box<dyn Error>> {
    let options = cli::parse(std::env::args().skip(1))?;

    let mut linker = Linker::new();
    for input in &options.inputs {
        linker.load_file(input)?;
    }

    if let Some(DebugPrint::MergedObject) = options.debug_print {
        println!("{:#?}", linker.loaded_object_for_debug_print());
        return Ok(());
    }

    let linker = linker.calculate_layout()?;

    if let Some(DebugPrint::Layout) = options.debug_print {
        println!("Section addresses");
        println!("-----------------");
        println!("{:#x?}", linker.section_addresses_for_debug_print());
        println!();
        println!("Section merges");
        println!("--------------");
        for merge in linker.section_merges_for_debug_print() {
            println!("{merge:#x?}");
        }
        return Ok(());
    }

    todo!();
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
