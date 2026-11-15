//! The file names of the artifacts a build writes per deployment (rsdl
//! reference §3.4).
//!
//! The rule lives here because its users depend on `ridl-ir`: the rsdl
//! checker, which rejects two deployments whose file names are equal when case
//! is ignored (RSDL-711), and `ridlc`, which writes the files. One function
//! keeps the check and the write in agreement. `ridl-core` also calls
//! [`is_reserved_device_name`] for the package-name rule (MANI-006), because
//! every artifact file name starts with the package name.

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
/// more of `a`–`z`, `0`–`9`, `.`, `_` and `-`, starting and ending with a
/// letter or a digit, and with no `..`. The part before the first `.` is not a
/// Windows reserved device name (`con`, `prn`, `aux`, `nul`, `com1` to `com9`,
/// `lpt1` to `lpt9`), because Windows opens the device for `con.anything` too.
/// The default stem ([`deployment_file_stem`]) starts with the package name,
/// and a package name whose first segment is a reserved device name is
/// MANI-006, so the default stem needs no check of its own.
pub fn is_valid_file_stem(stem: &str) -> bool {
    let mut chars = stem.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first.is_ascii_lowercase() || first.is_ascii_digit())
        && chars
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '-'))
        && stem.ends_with(|c: char| c.is_ascii_lowercase() || c.is_ascii_digit())
        && !stem.contains("..")
        && !is_reserved_device_name(stem.split('.').next().unwrap_or(stem))
}

/// Whether `name` is a Windows reserved device name. The caller passes one
/// lowercase segment with no `.`, such as the part of a file name before its
/// first `.`; the match is exact and case-sensitive.
pub fn is_reserved_device_name(name: &str) -> bool {
    // COM0 and LPT0 are not reserved in Microsoft's file-naming documentation.
    matches!(name, "con" | "prn" | "aux" | "nul")
        || ["com", "lpt"].iter().any(|prefix| {
            name.strip_prefix(prefix)
                .is_some_and(|rest| matches!(rest.as_bytes(), [b'1'..=b'9']))
        })
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
        for good in [
            "view-dev",
            "a",
            "0",
            "veh.cabin.bench",
            "a_b-c.9",
            "console",
            "com10",
            "nullx",
            "com0",
            "lpt0",
            "x.con",
        ] {
            assert!(is_valid_file_stem(good), "`{good}` is valid");
        }
        for bad in [
            "", "View", "view dev", "../x", "a..b", ".hidden", "-x", "_x", "a/b", "a\\b", "é",
            "a.", "a-", "a_", "viEw", "vieW", "con", "nul.x", "com1", "lpt9", "prn", "aux",
        ] {
            assert!(!is_valid_file_stem(bad), "`{bad}` is not valid");
        }
    }
}
