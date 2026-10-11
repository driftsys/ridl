//! The minimum supported Rust version and edition of the published runtime
//! crates, with workspace values resolved.

use super::rust_api::RUNTIME_CRATES;
use std::path::Path;
use toml::Value;

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

/// `<name> edition=<e>`, and `<name> rust-version=<v>` when the `[package]`
/// table sets a rust-version. A crate that gains a minimum Rust version
/// therefore only adds a line, and a crate that changes one removes a line.
fn crate_lines(name: &str, package: &Value, workspace: &Value) -> Result<Vec<String>, String> {
    let edition = resolve(package, workspace, "edition")
        .ok_or_else(|| format!("crates/{name}/Cargo.toml: no edition"))?;
    let mut lines = vec![format!("{name} edition={edition}")];
    if let Some(rust_version) = resolve(package, workspace, "rust-version") {
        lines.push(format!("{name} rust-version={rust_version}"));
    }
    Ok(lines)
}

/// The lines of [`crate_lines`] for each crate of [`RUNTIME_CRATES`].
pub fn dump(root: &Path) -> Result<Vec<String>, String> {
    let workspace = read_toml(&root.join("Cargo.toml"))?;
    let mut lines = Vec::new();
    for name in RUNTIME_CRATES {
        let manifest = read_toml(&root.join("crates").join(name).join("Cargo.toml"))?;
        let package = manifest
            .get("package")
            .ok_or_else(|| format!("crates/{name}/Cargo.toml has no [package]"))?;
        lines.extend(crate_lines(name, package, &workspace)?);
    }
    Ok(lines)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::freeze::missing_lines;

    fn text(package: &str) -> String {
        let package: Value = toml::from_str(package).unwrap();
        let workspace = Value::Table(toml::Table::new());
        crate_lines("c", &package, &workspace)
            .unwrap()
            .into_iter()
            .map(|l| l + "\n")
            .collect()
    }

    #[test]
    fn a_crate_gaining_a_minimum_rust_version_only_adds_a_line() {
        let baseline = text("edition = \"2021\"");
        let head = text("edition = \"2021\"\nrust-version = \"1.83\"");
        assert!(
            missing_lines(&baseline, &head).is_empty(),
            "{baseline}{head}"
        );
    }

    #[test]
    fn raising_the_minimum_rust_version_removes_a_line() {
        let baseline = text("edition = \"2021\"\nrust-version = \"1.83\"");
        let head = text("edition = \"2021\"\nrust-version = \"1.85\"");
        assert_eq!(
            missing_lines(&baseline, &head),
            vec!["c rust-version=1.83"],
            "{head}"
        );
    }

    #[test]
    fn msrv_dump_reads_ridl_rt() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let lines = dump(root).unwrap();
        assert!(
            lines.iter().any(|l| l == "ridl-rt rust-version=1.83"),
            "{lines:?}"
        );
        assert!(
            lines.iter().any(|l| l == "ridl-rt edition=2021"),
            "{lines:?}"
        );
    }
}
