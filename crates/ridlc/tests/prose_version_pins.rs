//! Guards the version pins that prose names. `git std bump` rewrites the
//! pins listed as `[[version_files]]` in `.git-std.toml`: for each entry it
//! replaces the first capture group of the first match of the pattern. It
//! skips an entry whose file or pattern does not match, without an error, so
//! a pin that stops matching goes stale at the next release and is found only
//! after the release commit is on main.
//!
//! This test applies each entry's own pattern to its file and requires every
//! match to carry the workspace version, so it fails in the pull request that
//! breaks a pin or an entry. It lives in `ridlc` because that crate already
//! holds the guard for the `ridl-rt` requirement it emits.
//!
//! `docs/design/flatbuffers-codec.md` names `<major.minor>` instead of a
//! version for the same literal, so it has no pin to guard.

use std::path::Path;

const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");

/// Files whose pins the release must keep current. Each needs at least one
/// `[[version_files]]` entry.
const REQUIRED_FILES: [&str; 2] = [
    "crates/ridl-rt-conformance/README.md",
    "docs/book/writing-a-port.md",
];

/// The pattern count `.git-std.toml` must hold for the files above: one for
/// the README and two for the book chapter.
const REQUIRED_ENTRIES: usize = 3;

#[derive(serde::Deserialize)]
struct VersionFile {
    path: String,
    regex: String,
}

fn read(relative: &str) -> String {
    std::fs::read_to_string(Path::new(ROOT).join(relative))
        .unwrap_or_else(|e| panic!("{relative} must exist and be readable: {e}"))
}

/// The workspace version from `[workspace.package]` in the root manifest.
fn workspace_version() -> String {
    #[derive(serde::Deserialize)]
    struct Root {
        workspace: Workspace,
    }
    #[derive(serde::Deserialize)]
    struct Workspace {
        package: Package,
    }
    #[derive(serde::Deserialize)]
    struct Package {
        version: String,
    }
    let root: Root = toml::from_str(&read("Cargo.toml")).expect("the root Cargo.toml is TOML");
    root.workspace.package.version
}

/// The `[[version_files]]` entries of `.git-std.toml`.
fn version_files() -> Vec<VersionFile> {
    #[derive(serde::Deserialize)]
    struct Config {
        #[serde(default)]
        version_files: Vec<VersionFile>,
    }
    let config: Config = toml::from_str(&read(".git-std.toml")).expect(".git-std.toml is TOML");
    config.version_files
}

#[test]
fn prose_version_pins_match_the_workspace_version() {
    let version = workspace_version();
    let entries = version_files();
    assert_eq!(
        entries.len(),
        REQUIRED_ENTRIES,
        ".git-std.toml must hold exactly {REQUIRED_ENTRIES} `[[version_files]]` entries; \
         update REQUIRED_ENTRIES in this test when an entry is added"
    );
    for required in REQUIRED_FILES {
        assert!(
            entries.iter().any(|e| e.path == required),
            "{required} needs a `[[version_files]]` entry in .git-std.toml, \
             or `git std bump` leaves its pin at the old version"
        );
    }
    for entry in &entries {
        let regex = regex::Regex::new(&entry.regex)
            .unwrap_or_else(|e| panic!("the pattern for {} is invalid: {e}", entry.path));
        let text = read(&entry.path);
        let pins: Vec<&str> = regex
            .captures_iter(&text)
            .map(|c| c.get(1).expect("a pattern has a capture group").as_str())
            .collect();
        assert!(
            !pins.is_empty(),
            "the pattern `{}` matches nothing in {}, so `git std bump` skips it; \
             repair the pattern or the pin",
            entry.regex,
            entry.path
        );
        for pin in pins {
            assert_eq!(
                pin, version,
                "{} holds the pin `{pin}` (pattern `{}`), but the workspace version is \
                 {version}; update the pin in {}",
                entry.path, entry.regex, entry.path
            );
        }
    }
}
