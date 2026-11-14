//! The file names of the artifacts a build writes per deployment (rsdl
//! reference §3.4).
//!
//! The rule lives here because both its users depend on `ridl-ir`: the rsdl
//! checker, which rejects two deployments whose file names are equal when case
//! is ignored (RSDL-711), and `ridlc`, which writes the files. One function
//! keeps the check and the write in agreement.

/// The suffix of a deployment descriptor's file name, after the stem.
pub const DEPLOYMENT_FILE_SUFFIX: &str = ".deployment.binfb";

/// The file stem of a deployment's descriptor, before the `.deployment.binfb`
/// suffix.
///
/// `file` is the value of the deployment's `file` attribute. When it is
/// present it is the stem as written. When it is absent the stem is
/// `<package>.<name>` in ASCII lowercase, so `Development` in
/// `com.ampere.viewif` gives `com.ampere.viewif.development`.
pub fn deployment_file_stem(package: &str, name: &str, file: Option<&str>) -> String {
    match file {
        Some(file) => file.to_string(),
        None => format!("{package}.{name}").to_ascii_lowercase(),
    }
}

/// Whether `stem` is a valid value of the `file` attribute (RSDL-710): one or
/// more of `a`–`z`, `0`–`9`, `.`, `_` and `-`, starting with a letter or a
/// digit, and with no `..`. The rule keeps the name lowercase and inside the
/// output directory.
pub fn is_valid_file_stem(stem: &str) -> bool {
    let mut chars = stem.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first.is_ascii_lowercase() || first.is_ascii_digit())
        && chars
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '-'))
        && !stem.contains("..")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_stem_is_the_qualified_name_in_lowercase() {
        assert_eq!(
            deployment_file_stem("com.ampere.viewif", "Development", None),
            "com.ampere.viewif.development"
        );
    }

    #[test]
    fn a_file_attribute_replaces_the_default_stem() {
        assert_eq!(
            deployment_file_stem("com.ampere.viewif", "Development", Some("view-dev")),
            "view-dev"
        );
    }

    #[test]
    fn a_valid_stem_is_lowercase_and_stays_in_the_directory() {
        for good in ["view-dev", "a", "0", "veh.cabin.bench", "a_b-c.9"] {
            assert!(is_valid_file_stem(good), "`{good}` is valid");
        }
        for bad in [
            "", "View", "view dev", "../x", "a..b", ".hidden", "-x", "_x", "a/b", "a\\b", "é",
        ] {
            assert!(!is_valid_file_stem(bad), "`{bad}` is not valid");
        }
    }
}
