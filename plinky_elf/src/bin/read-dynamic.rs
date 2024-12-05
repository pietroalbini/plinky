use plinky_diagnostics::widgets::{Table, Text, Widget};
use plinky_diagnostics::WidgetWriter;
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

    let mut widgets: Vec<Box<dyn Widget>> = Vec::new();

    if let Some(soname) = dynamic.soname()? {
        widgets.push(Box::new(Text::new(format!("soname: {soname}"))));
    }

    let mut symbols = Table::new();
    symbols.set_title("Dynamic symbols:");
    symbols.add_row(["Name", "Visibility", "Binding"]);
    for symbol in dynamic.symbols()? {
        let name = if symbol.name.is_empty() { "<empty name>" } else { symbol.name.as_str() };
        symbols.add_row([name.into(), symbol.visibility.to_string(), symbol.binding.to_string()]);
    }
    widgets.push(Box::new(symbols));

    println!("{}", MultipleWidgets(widgets).render_to_string());

    Ok(())
}

struct MultipleWidgets(Vec<Box<dyn Widget>>);

impl Widget for MultipleWidgets {
    fn render(&self, writer: &mut WidgetWriter<'_>) {
        let mut first = true;
        for widget in &self.0 {
            if first {
                first = false;
            } else {
                writer.push_str("\n\n");
            }
            widget.render(writer);
        }
    }
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
