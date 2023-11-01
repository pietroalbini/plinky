use std::error::Error;
use std::process::ExitCode;

mod cli;

fn app() -> Result<(), Box<dyn Error>> {
    let options = cli::parse(std::env::args().skip(1))?;
    dbg!(options);

    Ok(())
}

fn main() -> ExitCode {
    match app() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("error: {err}");

            let mut source = err.source();
            while let Some(s) = source {
                eprintln!("cause: {s}");
                source = s.source();
            }

            ExitCode::FAILURE
        }
    }
}
