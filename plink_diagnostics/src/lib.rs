mod table;

pub use crate::table::Table;

#[cfg(test)]
#[must_use]
fn configure_insta() -> impl Drop {
    use insta::Settings;

    let mut settings = Settings::clone_current();
    settings.set_snapshot_path(concat!(env!("CARGO_MANIFEST_DIR"), "/snapshots"));

    settings.bind_to_scope()
}
