//! The example and corpus packages that `ridl check` accepts.

use super::bin;
use std::path::Path;

/// `<relative path> ok` for each tracked package directory under `examples/`
/// and `evals/corpus/` on which `ridl check` exits 0.
pub fn dump(root: &Path) -> Result<Vec<String>, String> {
    let ridl = bin::binary(root, "ridl")?;
    let mut lines = Vec::new();
    for dir in bin::package_dirs(root, &["examples", "evals/corpus"])? {
        if bin::run(&ridl, root, &["check", &dir])?.code == 0 {
            lines.push(format!("{dir} ok"));
        }
    }
    Ok(lines)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sources_dump_accepts_cabin() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let lines = dump(root).unwrap();
        assert!(lines.iter().any(|l| l == "examples/cabin ok"), "{lines:?}");
    }
}
