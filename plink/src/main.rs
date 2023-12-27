use crate::debug_print::DebugCallbacks;
use crate::linker::{link_driver, LinkerError};
use std::error::Error;
use std::process::ExitCode;

mod cli;
mod debug_print;
mod interner;
mod linker;
mod passes;
mod repr;

fn app() -> Result<(), Box<dyn Error>> {
    let options = cli::parse(std::env::args().skip(1))?;

    let callbacks = DebugCallbacks { print: options.debug_print.clone() };
    match link_driver(&options, &callbacks) {
        Ok(()) => {}
        Err(LinkerError::CallbackEarlyExit) => {}
        Err(err) => return Err(err.into()),
    }

    Ok(())
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
