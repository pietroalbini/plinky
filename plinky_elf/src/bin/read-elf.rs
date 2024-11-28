use plinky_diagnostics::widgets::Widget;
use plinky_elf::ElfReader;
use plinky_elf::render_elf::RenderElfFilters;
use std::error::Error;
use std::fs::File;

fn actual_main(args: &[String]) -> Result<(), Box<dyn Error>> {
    let (path, filters) = match args {
        [path] => (path, RenderElfFilters::all()),
        [path, filters] => (path, RenderElfFilters::parse(filters)?),
        _ => usage(),
    };

    let mut file = File::open(path)?;
    let object = ElfReader::new(&mut file)?.into_object()?;

    println!("{}", plinky_elf::render_elf::render(&object, &filters).render_to_string());

    Ok(())
}

fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if let Err(err) = actual_main(&args) {
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
