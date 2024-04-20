//! Check whether ELF files are parsed correctly.

use anyhow::{Context, Error};
use plinky_diagnostics::widgets::Widget;
use plinky_elf::ids::serial::SerialIds;
use plinky_elf::render_elf::RenderElfFilters;
use plinky_elf::ElfObject;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::process::Command;
use tempfile::NamedTempFile;

macro_rules! test {
    ($name:ident, $source:expr $(, $variant:ident)*$(,)?) => {
        mod $name {
            use super::*;

            $(
                #[test]
                #[allow(non_snake_case)]
                fn $variant() -> Result<(), Error> {
                    implement_test($source, stringify!($variant))
                }
            )*
        }
    };
}

test!(
    hello_asm,
    "tests/snapshot/hello_asm.asm",
    x86,
    x86_64,
    x86__linked,
    x86_64__linked,
    x86__written,
    x86_64__written,
);
test!(hello_c, "tests/snapshot/hello_c.c", x86, x86_64);

#[track_caller]
fn implement_test(source: &str, name: &str) -> Result<(), Error> {
    let meta = Metadata::from_name(name);

    let mut object_file = match source.rsplit_once('.').map(|(_name, ext)| ext) {
        Some("asm") => compile_asm(source, meta.variant)?,
        Some("c") => compile_c(source, meta.variant)?,
        Some(other) => panic!("unsupported extension: {other}"),
        None => panic!("missing extension for {source}"),
    };
    if let Link::Yes = meta.link {
        object_file = link_single_object(object_file.path(), meta.variant)?;
    }
    let mut file = BufReader::new(File::open(object_file.path())?);

    let mut parsed = ElfObject::load(&mut file, &mut SerialIds::new())?;

    let mut settings = insta::Settings::clone_current();
    settings.set_omit_expression(true);
    settings.set_snapshot_path("snapshot");
    settings.set_prepend_module_to_snapshot(false);
    settings.set_snapshot_suffix(meta.snapshot_suffix());
    let _guard = settings.bind_to_scope();

    let name = source.rsplit_once('/').map(|(_dir, name)| name).unwrap_or(source);
    let name = name.rsplit_once('.').map(|(name, _ext)| name).unwrap_or(name);

    match meta.mode {
        Mode::Read => {}
        Mode::WriteThenRead => {
            let mut buf = Vec::new();
            parsed.write(&mut buf).context("failed to write back the ELF file")?;
            parsed = ElfObject::load(&mut std::io::Cursor::new(&mut buf), &mut SerialIds::new())?;
        }
    }

    let rendered =
        plinky_elf::render_elf::render(&parsed, &RenderElfFilters::all()).render_to_string();
    insta::assert_snapshot!(name, rendered);
    Ok(())
}

fn compile_asm(source: &str, variant: Variant) -> Result<NamedTempFile, Error> {
    let dest = NamedTempFile::new()?;
    let status = Command::new("nasm")
        .arg("-o")
        .arg(dest.path())
        .arg("-f")
        .arg(match variant {
            Variant::X86 => "elf32",
            Variant::X86_64 => "elf64",
        })
        .arg(source)
        .status()?;
    anyhow::ensure!(status.success(), "failed to compile {}", source);
    Ok(dest)
}

fn compile_c(source: &str, variant: Variant) -> Result<NamedTempFile, Error> {
    let dest = NamedTempFile::new()?;
    let status = Command::new("gcc")
        .arg("-c")
        .arg("-o")
        .arg(dest.path())
        .arg(source)
        .arg("-fno-pic")
        .arg(match variant {
            Variant::X86 => "-m32",
            Variant::X86_64 => "-m64",
        })
        .status()?;
    anyhow::ensure!(status.success(), "failed to compile {}", source);
    Ok(dest)
}

fn link_single_object(object: &Path, variant: Variant) -> Result<NamedTempFile, Error> {
    let dest = NamedTempFile::new()?;
    let status = Command::new("ld")
        .arg("-o")
        .arg(dest.path())
        .arg(object)
        .arg("-znoexecstack")
        .args(match variant {
            Variant::X86 => ["-m", "elf_i386"],
            Variant::X86_64 => ["-m", "elf_x86_64"],
        })
        .status()?;
    anyhow::ensure!(status.success(), "failed to link {object:?}");
    Ok(dest)
}

#[derive(Clone, Copy)]
struct Metadata {
    variant: Variant,
    link: Link,
    mode: Mode,
}

impl Metadata {
    fn from_name(name: &str) -> Metadata {
        fn set<T>(component: &str, store: &mut Option<T>, value: T) {
            if store.is_some() {
                panic!("duplicate {component} in name");
            }
            *store = Some(value);
        }

        let mut variant = None;
        let mut link = None;
        let mut mode = None;

        for component in name.split("__") {
            match component {
                "x86" => set(component, &mut variant, Variant::X86),
                "x86_64" => set(component, &mut variant, Variant::X86_64),
                "linked" => set(component, &mut link, Link::Yes),
                "written" => set(component, &mut mode, Mode::WriteThenRead),
                other => panic!("unknown component {other}"),
            }
        }

        Metadata {
            variant: variant.expect("missing variant"),
            link: link.unwrap_or(Link::No),
            mode: mode.unwrap_or(Mode::Read),
        }
    }

    fn snapshot_suffix(&self) -> String {
        let mut suffix = match self.variant {
            Variant::X86 => "32bit".to_string(),
            Variant::X86_64 => "64bit".to_string(),
        };
        if let Link::Yes = self.link {
            suffix.push_str("-linked");
        }
        if let Mode::WriteThenRead = self.mode {
            suffix.push_str("-written");
        }
        suffix
    }
}

#[derive(Clone, Copy)]
enum Variant {
    X86,
    X86_64,
}

#[derive(Clone, Copy)]
enum Link {
    Yes,
    No,
}

#[derive(Clone, Copy)]
enum Mode {
    Read,
    WriteThenRead,
}
