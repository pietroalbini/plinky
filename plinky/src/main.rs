#![feature(error_generic_member_access)]
#![feature(array_windows)]

use crate::debug_print::DebugCallbacks;
use crate::linker::link_driver;
use plinky_diagnostics::Diagnostic;
use std::error::{Error, request_ref};
use std::process::ExitCode;

mod arch;
mod cli;
mod debug_print;
mod diagnostics;
mod interner;
mod linker;
mod passes;
mod repr;
mod utils;

fn app() -> Result<(), Box<dyn Error>> {
    let options = cli::parse(std::env::args().skip(1))?;

    let callbacks = DebugCallbacks { print: options.debug_print.clone() };
    link_driver(&options, &callbacks)?;

    Ok(())
}

fn render_error(err: Box<dyn Error>) -> ExitCode {
    let mut diagnostic = None;
    let mut current: Option<&(dyn Error + 'static)> = Some(&*err);
    while let Some(current_err) = current {
        if let Some(extracted) = request_ref::<Diagnostic>(current_err) {
            diagnostic = Some(extracted);
            break;
        }
        current = current_err.source();
    }

    if let Some(diagnostic) = diagnostic {
        eprintln!("{diagnostic}");
    } else {
        eprintln!("error: {err}");

        let mut source = err.source();
        while let Some(s) = source {
            eprintln!("caused by: {s}");
            source = s.source();
        }
    }

    ExitCode::FAILURE
}

fn main() -> ExitCode {
    match app() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => render_error(err),
    }
}
