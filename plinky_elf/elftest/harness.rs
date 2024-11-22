use anyhow::{bail, Error};
use plinky_elf::writer::layout::Layout;
use plinky_elf::writer::Writer;
use plinky_elf::ElfReader;
use plinky_test_harness::template::Template;
use plinky_test_harness::{Step, TestContext};
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ReadElfStep {
    file: Template,
    #[serde(default = "default_true")]
    roundtrip: bool,
    #[serde(default)]
    filter: Option<String>,
}

impl Step for ReadElfStep {
    fn run(&self, ctx: TestContext<'_>) -> Result<(), Error> {
        insta::allow_duplicates! {
            let file = ctx.maybe_relative_to_src(&self.file.resolve(&*ctx.template)?);
            self.read(&ctx, &file)?;

            if self.roundtrip {
                let roundtrip = self.roundtrip(&ctx, &file)?;
                self.read(&ctx, &roundtrip)?;
            }

            Ok::<(), Error>(())
        }
    }

    fn templates(&self) -> Vec<Template> {
        vec![self.file.clone()]
    }

    fn is_leaf(&self) -> bool {
        true
    }
}

impl ReadElfStep {
    fn read(&self, ctx: &TestContext<'_>, file: &Path) -> Result<(), Error> {
        println!("reading {}...", file.display());

        let mut command = Command::new(env!("CARGO_BIN_EXE_read-elf"));
        command.arg(file);
        if let Some(filter) = &self.filter {
            command.arg(filter);
        }

        let mut runner = ctx.run_and_snapshot();
        let outcome = runner.run("reading ELF", &mut command)?;
        runner.persist();

        if !outcome {
            bail!("failed to read the ELF file");
        }

        Ok(())
    }

    fn roundtrip(&self, ctx: &TestContext<'_>, file: &Path) -> Result<PathBuf, Error> {
        println!("writing the file back for the roundtrip...");

        let dest = ctx.dest.join(ctx.step_name).join("roundtrip").join(file.file_name().unwrap());
        std::fs::create_dir_all(dest.parent().unwrap())?;

        let object = ElfReader::new(&mut BufReader::new(File::open(file)?))?.into_object()?;
        Writer::new(
            &mut BufWriter::new(File::create_new(&dest)?),
            &object,
            Layout::new(&object, None)?,
        )?
        .write()?;

        Ok(dest)
    }
}

fn default_true() -> bool {
    true
}

fn main() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("elftest");
    plinky_test_harness::main(&path, |steps| {
        steps.define_builtins()?.define::<ReadElfStep>("read-elf")
    });
}
