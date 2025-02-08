use crate::{ElfABI, ElfClass, ElfEndian, ElfMachine, ElfObject, ElfType};
use plinky_diagnostics::widgets::{Table, Widget};

pub(super) fn render_meta(object: &ElfObject) -> impl Widget + use<> {
    let mut table = Table::new();
    table.set_title("Metadata:");
    table.add_body(["Class", match object.env.class {
        ElfClass::Elf32 => "ELF 32bit",
        ElfClass::Elf64 => "ELF 64bit",
    }]);
    table.add_body(["Endian", match object.env.endian {
        ElfEndian::Little => "Little",
    }]);
    table.add_body(["ABI", match object.env.abi {
        ElfABI::SystemV => "System V",
    }]);
    table.add_body(["Machine", match object.env.machine {
        ElfMachine::X86 => "x86",
        ElfMachine::X86_64 => "x86-64",
    }]);
    table.add_body(["Type", match object.type_ {
        ElfType::Relocatable => "Relocatable",
        ElfType::Executable => "Executable",
        ElfType::SharedObject => "Shared object",
        ElfType::Core => "Core dump",
    }]);
    table.add_body(["Entrypoint".to_string(), match object.entry {
        Some(entry) => format!("{entry:#x}"),
        None => "-".to_string(),
    }]);
    table
}
