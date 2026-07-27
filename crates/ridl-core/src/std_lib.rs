//! The embedded `ridl.std` package (typl reference Appendix A, ADR-0007
//! decision 15).
//!
//! The Appendix A source is committed verbatim as
//! `crates/ridl-core/assets/ridl_std.typl` and compiled in via `include_str!`
//! — a built-in, implicitly imported package with no filesystem or network
//! lookup, version-locked to the compiler binary.

use crate::db::{InputFile, RidlDatabase};
use crate::package::{Package, PackageOrigin};

/// The `ridl.std` source, verbatim from the typl reference Appendix A.
pub const RIDL_STD_SOURCE: &str = include_str!("../assets/ridl_std.typl");

/// The virtual path the embedded source is registered under. The `<builtin>`
/// prefix marks it as compiler-provided, not a filesystem path.
pub const RIDL_STD_PATH: &str = "<builtin>/ridl_std.typl";

/// The built-in `ridl.std` package, created at most once per database.
///
/// The first call registers the embedded source as an [`InputFile`] and wraps
/// it in a [`Package`] with [`PackageOrigin::Std`]; every later call on the
/// same database returns that same package.
pub fn std_package(db: &mut RidlDatabase) -> Package {
    if let Some(&package) = db.std_package_cache.get() {
        return package;
    }
    let file = InputFile::new(&*db, RIDL_STD_PATH.to_string(), RIDL_STD_SOURCE.to_string());
    let package = Package::new(
        &*db,
        "ridl.std".to_string(),
        vec![file],
        PackageOrigin::Std,
        std::collections::BTreeMap::new(),
        None,
    );
    let _ = db.std_package_cache.set(package);
    package
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::parse_file;

    /// Appendix A is normative and the embedded copy must parse clean under
    /// the current parser — a parse error here means the asset and the parser
    /// have drifted apart.
    #[test]
    fn std_package_parses_clean_and_exposes_ridl_std() {
        let mut db = RidlDatabase::default();
        let package = std_package(&mut db);

        assert_eq!(package.name(&db).as_str(), "ridl.std");
        assert_eq!(*package.origin(&db), PackageOrigin::Std);
        assert!(
            package.imports(&db).is_empty(),
            "the built-in package has no manifest and no imports",
        );

        let files = package.files(&db);
        assert_eq!(files.len(), 1, "ridl.std is a single embedded file");
        assert_eq!(files[0].path(&db).as_str(), RIDL_STD_PATH);

        let parse = parse_file(&db, files[0]);
        assert_eq!(
            parse.errors(),
            &[],
            "the embedded Appendix A source must parse without errors",
        );

        let declared = parse.syntax().text().to_string();
        assert_eq!(declared, RIDL_STD_SOURCE, "the parse is lossless");
        assert!(
            RIDL_STD_SOURCE.starts_with("package ridl.std\n"),
            "the asset starts with the `package ridl.std` declaration",
        );
    }

    #[test]
    fn std_package_is_memoized_per_database() {
        let mut db = RidlDatabase::default();
        let first = std_package(&mut db);
        let second = std_package(&mut db);
        assert_eq!(first, second, "the same database returns the same package");
    }

    /// The asset is the normative Appendix A, committed verbatim. Nothing
    /// enforced that until this test: both were edited by hand in #198, and
    /// every gate would have passed had only one been.
    ///
    /// This matters more since the standard package became a shipped artifact
    /// (issue #190). An asset that has drifted from the appendix now generates
    /// code that disagrees with the specification.
    #[test]
    fn the_asset_is_appendix_a_verbatim() {
        let reference = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../docs/specification/typl-language-reference.md"
        ))
        .expect("the typl reference is readable");

        let block = reference
            .split("```ridl\n")
            .find(|block| block.starts_with("package ridl.std\n"))
            .and_then(|block| block.split("\n```").next())
            .expect("Appendix A carries a `ridl.std` fenced block");

        assert_eq!(
            format!("{block}\n"),
            RIDL_STD_SOURCE,
            "`crates/ridl-core/assets/ridl_std.typl` and Appendix A of \
             `docs/specification/typl-language-reference.md` have drifted apart. \
             They are the same normative text and are edited together.",
        );
    }
}
