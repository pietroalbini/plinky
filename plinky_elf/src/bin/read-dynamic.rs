use plinky_diagnostics::widgets::{Table, Widget};
use plinky_elf::ElfReader;
use std::error::Error;
use std::fs::File;

fn actual_main(args: &[String]) -> Result<(), Box<dyn Error>> {
    let path = match args {
        [path] => path,
        _ => usage(),
    };

    let mut file = File::open(path)?;
    let mut reader = ElfReader::new(&mut file)?;

    let mut dynamic = reader.dynamic()?;

    let mut symbols = Table::new();
    symbols.set_title("Dynamic symbols:");
    for name in dynamic.symbol_names()? {
        if name.is_empty() {
            symbols.add_row(["<empty name>"]);
        } else {
            symbols.add_row([name]);
        }
    }

    println!("{}", symbols.render_to_string());

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
