use plink_elf::errors::LoadError;
use plink_elf::Object;
use std::error::Error;
use std::fs::File;
use std::path::Path;

fn actual_main(path: &Path) -> Result<(), LoadError> {
    let mut file = File::open(path)?;
    let object = Object::load(&mut file)?;

    println!("{object:#?}");

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
