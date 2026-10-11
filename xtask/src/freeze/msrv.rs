//! The minimum supported Rust version and edition of the published runtime
//! crates, with workspace values resolved.

use std::path::Path;
use toml::Value;

const CRATES: [&str; 4] = [
    "ridl-rt",
    "ridl-rt-conformance",
    "ridl-loopback",
    "ridl-descriptor",
];

fn read_toml(path: &Path) -> Result<Value, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
}

/// The value of `key` in a crate's `[package]`, resolved against the root
/// `[workspace.package]` when the crate writes `<key>.workspace = true`.
fn resolve(package: &Value, workspace: &Value, key: &str) -> Option<String> {
    match package.get(key)? {
        Value::String(s) => Some(s.clone()),
        Value::Table(t) if t.get("workspace").and_then(Value::as_bool) == Some(true) => workspace
            .get("workspace")?
            .get("package")?
            .get(key)?
            .as_str()
            .map(str::to_string),
        _ => None,
    }
}

/// `<crate> rust-version=<v> edition=<e>`; `rust-version=none` when the crate
/// sets none.
pub fn dump(root: &Path) -> Result<Vec<String>, String> {
    let workspace = read_toml(&root.join("Cargo.toml"))?;
    let mut lines = Vec::new();
    for name in CRATES {
        let manifest = read_toml(&root.join("crates").join(name).join("Cargo.toml"))?;
        let package = manifest
            .get("package")
            .ok_or_else(|| format!("crates/{name}/Cargo.toml has no [package]"))?;
        let rust_version = resolve(package, &workspace, "rust-version").unwrap_or("none".into());
        let edition = resolve(package, &workspace, "edition")
            .ok_or_else(|| format!("crates/{name}/Cargo.toml: no edition"))?;
        lines.push(format!(
            "{name} rust-version={rust_version} edition={edition}"
        ));
    }
    Ok(lines)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn msrv_dump_reads_ridl_rt() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let lines = dump(root).unwrap();
        assert!(
            lines
                .iter()
                .any(|l| l == "ridl-rt rust-version=1.83 edition=2021"),
            "{lines:?}"
        );
    }
}
