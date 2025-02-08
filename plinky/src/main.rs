#![feature(error_generic_member_access)]
#![feature(array_windows)]

use crate::debug_print::DebugCallbacks;
use crate::linker::link_driver;
use plinky_diagnostics::{DiagnosticBuilder, DiagnosticContext, GatheredContext};
use std::error::{request_ref, Error};
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
    let mut diagnostic_builder = None;
    let mut diagnostic_context = GatheredContext::new();
    let mut current: Option<&(dyn Error + 'static)> = Some(&*err);
    while let Some(current_err) = current {
        if let Some(extracted) = request_ref::<dyn DiagnosticBuilder>(current_err) {
            if diagnostic_builder.is_none() {
                diagnostic_builder = Some(extracted);
            }
        }
        if let Some(extracted) = request_ref::<dyn DiagnosticContext>(current_err) {
            diagnostic_context.add(extracted);
        }
        current = current_err.source();
    }

    if let Some(diagnostic_builder) = diagnostic_builder {
        let diagnostic = diagnostic_builder.build(&diagnostic_context);
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
