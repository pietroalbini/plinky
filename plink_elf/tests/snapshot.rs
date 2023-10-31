//! Check whether ELF files are parsed correctly.

use anyhow::Error;
use plink_elf::Object;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::process::Command;
use tempfile::NamedTempFile;

macro_rules! test {
    ($name:ident, $source:expr $(, $variant:ident)*) => {
        mod $name {
            use super::*;

            $(
                #[test]
                fn $variant() -> Result<(), Error> {
                    implement_test($source, stringify!($variant))
                }
            )*
        }
    };
}

test!(
    hello_asm,
    "tests/snapshot/hello_asm.S",
    x86,
    x86_64,
    x86_linked,
    x86_64_linked
);
test!(hello_c, "tests/snapshot/hello_c.c", x86, x86_64);

#[track_caller]
fn implement_test(source: &str, variant: &str) -> Result<(), Error> {
    let (variant, link) = match variant {
        "x86" => (Variant::X86, Link::No),
        "x86_64" => (Variant::X86_64, Link::No),
        "x86_linked" => (Variant::X86, Link::Yes),
        "x86_64_linked" => (Variant::X86_64, Link::Yes),
        other => panic!("unsupported variant: {other}"),
    };

    let mut object_file = match source.rsplit_once('.').map(|(_name, ext)| ext) {
        Some("S") => compile_asm(source, variant)?,
        Some("c") => compile_c(source, variant)?,
        Some(other) => panic!("unsupported extension: {other}"),
        None => panic!("missing extension for {source}"),
    };
    if let Link::Yes = link {
        object_file = link_single_object(object_file.path(), variant)?;
    }
    let mut file = BufReader::new(File::open(object_file.path())?);

    let parsed = Object::load(&mut file)?;

    let mut settings = insta::Settings::clone_current();
    settings.set_omit_expression(true);
    settings.set_snapshot_path("snapshot");
    settings.set_prepend_module_to_snapshot(false);
    settings.set_snapshot_suffix(match (variant, link) {
        (Variant::X86, Link::No) => "32bit",
        (Variant::X86_64, Link::No) => "64bit",
        (Variant::X86, Link::Yes) => "32bit-linked",
        (Variant::X86_64, Link::Yes) => "64bit-linked",
    });
    let _guard = settings.bind_to_scope();

    let name = source
        .rsplit_once('/')
        .map(|(_dir, name)| name)
        .unwrap_or(source);
    let name = name
        .rsplit_once('.')
        .map(|(name, _ext)| name)
        .unwrap_or(name);

    insta::assert_snapshot!(name, format!("{parsed:#x?}"));
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
        .args(match variant {
            Variant::X86 => ["-m", "elf_i386"],
            Variant::X86_64 => ["-m", "elf_x86_64"],
        })
        .status()?;
    anyhow::ensure!(status.success(), "failed to link {object:?}");
    Ok(dest)
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
