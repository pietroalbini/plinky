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
