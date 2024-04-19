use plinky_diagnostics::widgets::Widget;
use plinky_elf::errors::LoadError;
use plinky_elf::ids::serial::SerialIds;
use plinky_elf::{render_elf, ElfObject};
use std::error::Error;
use std::fs::File;
use std::path::Path;

fn actual_main(path: &Path) -> Result<(), LoadError> {
    let mut file = File::open(path)?;
    let object = ElfObject::load(&mut file, &mut SerialIds::new())?;

    println!("{}", render_elf(&object).render_to_string());

    Ok(())
}

fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    if args.len() != 2 {
        usage();
    }
    let path = Path::new(&args[1]);

    if let Err(err) = actual_main(path) {
        eprintln!("error: {err}");

        let mut source = err.source();
        while let Some(s) = source {
            eprintln!("  cause: {s}");
            source = s.source();
        }

        std::process::exit(1);
    }
}

fn usage() -> ! {
    eprintln!("usage: read-elf <path>");
    std::process::exit(1);
}
