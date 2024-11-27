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
    symbols.add_row(["Name", "Visibility", "Binding"]);
    for symbol in dynamic.symbols()? {
        let name = if symbol.name.is_empty() { "<empty name>" } else { symbol.name.as_str() };
        symbols.add_row([name.into(), symbol.visibility.to_string(), symbol.binding.to_string()]);
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
