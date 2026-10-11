//! The example and corpus packages that `ridl check` accepts, read from their
//! frozen copies under [`bin::SOURCE_COPIES`].

use super::bin;
use std::path::Path;

/// Each package the dump checks, as `(label, directory)`: the directory is the
/// tracked copy that `ridl check` reads, relative to the repository root, and
/// the label is the path of the original, which the dump line names.
fn packages(root: &Path) -> Result<Vec<(String, String)>, String> {
    let prefix = format!("{}/", bin::SOURCE_COPIES);
    bin::package_dirs(root, &[bin::SOURCE_COPIES])?
        .into_iter()
        .map(|dir| {
            let label = dir
                .strip_prefix(&prefix)
                .ok_or_else(|| format!("{dir} is not under {prefix}"))?
                .to_string();
            Ok((label, dir))
        })
        .collect()
}

/// `<path> ok` for each package copy on which `ridl check` exits 0, where
/// `<path>` is the path of the original, for example `examples/cabin`.
pub fn dump(root: &Path) -> Result<Vec<String>, String> {
    let ridl = bin::binary(root, "ridl")?;
    let mut lines = Vec::new();
    for (label, dir) in packages(root)? {
        if bin::run(&ridl, root, &["check", &dir])?.code == 0 {
            lines.push(format!("{label} ok"));
        }
    }
    Ok(lines)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> &'static Path {
        Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap()
    }

    #[test]
    fn sources_dump_accepts_cabin() {
        let lines = dump(root()).unwrap();
        assert!(lines.iter().any(|l| l == "examples/cabin ok"), "{lines:?}");
    }

    #[test]
    fn sources_dump_reads_the_frozen_copies() {
        // An edit to `examples/` or to `evals/corpus/` must not change the
        // dump, so each package is read from its copy.
        let packages = packages(root()).unwrap();
        assert!(
            packages.iter().any(|(label, dir)| label == "examples/cabin"
                && dir == "xtask/tests/freeze-fixtures/sources/examples/cabin"),
            "{packages:?}"
        );
        for (label, dir) in &packages {
            assert_eq!(
                *dir,
                format!("xtask/tests/freeze-fixtures/sources/{label}"),
                "{packages:?}"
            );
        }
    }
}
