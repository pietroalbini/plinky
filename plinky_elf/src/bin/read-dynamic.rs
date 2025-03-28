use plinky_diagnostics::WidgetWriter;
use plinky_diagnostics::widgets::{Table, Text, Widget};
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
    symbols.add_head(["Name", "Visibility", "Binding", "Defined"]);
    for symbol in dynamic.symbols()? {
        let name = if symbol.name.is_empty() { "<empty name>" } else { symbol.name.as_str() };
        let defined = if symbol.defined { "Yes" } else { "No" };
        symbols.add_body([
            name.into(),
            symbol.visibility.to_string(),
            symbol.binding.to_string(),
            defined.to_string(),
        ]);
    }
    widgets.push(Box::new(symbols));

    let needed = dynamic.needed_libraries()?;
    if !needed.is_empty() {
        let mut table = Table::new();
        table.set_title("Needed libraries:");
        for library in needed {
            table.add_body([library]);
        }
        widgets.push(Box::new(table));
    }

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
