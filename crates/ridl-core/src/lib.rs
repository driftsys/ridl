//! Placeholder — the resolver + checker land across epic E0 (docs/ROADMAP.md).

/// Returns this crate's name.
pub fn crate_name() -> &'static str {
    env!("CARGO_PKG_NAME")
}

#[test]
fn crate_name_matches_package() {
    assert_eq!(crate_name(), "ridl-core");
}
