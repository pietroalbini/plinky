use plinky_elf::ElfPermissions;

pub(super) fn permissions(perms: &ElfPermissions) -> String {
    let mut output = String::new();
    if perms.read {
        output.push('r');
    }
    if perms.write {
        output.push('w');
    }
    if perms.execute {
        output.push('x');
    }
    if output.is_empty() { "no perms".into() } else { format!("perms: {output}") }
}
