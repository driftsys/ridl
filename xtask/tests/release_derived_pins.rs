//! The check that the version pins derived from the workspace version follow
//! it.
//!
//! `git std bump` rewrites the root `Cargo.toml` and the `[[version_files]]`
//! entries of `.git-std.toml` (`crates/ridlc/tests/prose_version_pins.rs`
//! guards those). Some pins cannot be a `version_files` entry: a lockfile
//! holds one entry per crate, and `version_files` rewrites only the first
//! match, and the `ridl-rt` requirement that `ridl build` emits holds
//! `<major>.<minor>`, not the full version. `just release-sync` updates them
//! after the bump. This test fails when one of them does not follow the
//! workspace version; `just release` runs it before it commits and tags.
//!
//! The pins, with `<mm>` the `<major>.<minor>` of the workspace version, or the
//! full version for a pre-release, which only a requirement that names the
//! same pre-release matches:
//!
//! - `Cargo.lock`: every package with no `source` (the workspace members) is
//!   at the workspace version.
//! - `examples/cabin/Cargo.lock`: every `ridl-` package with no `source` (the
//!   crates of this workspace it resolves by path, `ridl-rt`, `ridl-loopback`,
//!   `ridl-transport-ws` and `ridl-descriptor` among them) is at the workspace
//!   version.
//! - `crates/ridlc/src/lib.rs`: `RIDL_RT_REQUIREMENT` is `<mm>`.
//! - the cabin consumer and server manifests, and the two manifests under
//!   `freeze/generated/`: the `ridl-rt` requirement is `<mm>`.
//! - `examples/cabin/Cargo.toml`: the comment that names the generated
//!   manifest's requirement says `<mm>`.
//! - `examples/cabin/web/deno.lock`: the `ridl-runtime` link is at the
//!   workspace version.
//! - `editors/vscode/package.json` and the two root versions in its
//!   `package-lock.json`: the editor version follows the workspace version,
//!   because `vscode-release.yaml` refuses an `editor-v` tag that differs from
//!   it.
//!
//! A pin whose file or line is not found is reported too, so a renamed file
//! or a rewritten line cannot pass without notice. The checking logic takes a
//! directory and a version, so the unit tests at the end feed it small
//! synthetic trees.

use std::path::Path;

const RIDL_RT_REQUIREMENT_PREFIX: &str = "const RIDL_RT_REQUIREMENT: &str = \"";
const CABIN_COMMENT_PREFIX: &str = "requires `ridl-rt = \"";
const RIDL_RUNTIME_LINK_PREFIX: &str = "jsr:@driftsys/ridl-runtime@";

/// The manifests whose `ridl-rt` dependency carries the `<mm>` requirement.
const RIDL_RT_MANIFESTS: [&str; 4] = [
    "examples/cabin/consumer/Cargo.toml",
    "examples/cabin/server/Cargo.toml",
    "freeze/generated/cabin/Cargo.toml",
    "freeze/generated/veh-cluster/Cargo.toml",
];

/// The crates of this workspace that `examples/cabin/Cargo.lock` must resolve
/// by path. Any other `ridl-` path package there is checked too.
const CABIN_PATH_CRATES: [&str; 4] = [
    "ridl-rt",
    "ridl-loopback",
    "ridl-transport-ws",
    "ridl-descriptor",
];

/// Returns one message for each pin under `root` that does not follow
/// `version`, naming the file and the value found.
fn derived_pin_errors(root: &Path, version: &str) -> Vec<String> {
    // `0.15` for `0.15.0`, but the full version for a pre-release such as
    // `0.15.0-rc.1`, which a `0.15` requirement does not match.
    let major_minor = if version.contains('-') {
        version.to_string()
    } else {
        version.splitn(3, '.').take(2).collect::<Vec<_>>().join(".")
    };
    let major_minor = major_minor.as_str();
    let mut errors = Vec::new();
    let mut check = |file: &str, found: Option<String>, expected: &str| match found {
        Some(found) if found == expected => {}
        Some(found) => errors.push(format!("{file}: found {found}, expected {expected}")),
        None => errors.push(format!(
            "{file}: the pin was not found (expected {expected})"
        )),
    };

    let lock = read_toml(root, "Cargo.lock");
    let packages = lock_packages(lock.as_ref());
    if packages.is_empty() {
        check("Cargo.lock", None, version);
    }
    for package in &packages {
        if package.get("source").is_none() {
            let name = package.get("name").and_then(|v| v.as_str()).unwrap_or("?");
            check(
                &format!("Cargo.lock (package {name})"),
                package
                    .get("version")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                version,
            );
        }
    }

    let cabin_lock = read_toml(root, "examples/cabin/Cargo.lock");
    let cabin_packages = lock_packages(cabin_lock.as_ref());
    let cabin_path_packages: Vec<(&str, Option<String>)> = cabin_packages
        .iter()
        .filter(|package| package.get("source").is_none())
        .filter_map(|package| {
            let name = package.get("name")?.as_str()?;
            let found = package
                .get("version")
                .and_then(|v| v.as_str())
                .map(String::from);
            name.starts_with("ridl-").then_some((name, found))
        })
        .collect();
    for crate_name in CABIN_PATH_CRATES {
        if !cabin_path_packages
            .iter()
            .any(|(name, _)| *name == crate_name)
        {
            check(
                &format!("examples/cabin/Cargo.lock (package {crate_name})"),
                None,
                version,
            );
        }
    }
    for (crate_name, found) in &cabin_path_packages {
        check(
            &format!("examples/cabin/Cargo.lock (package {crate_name})"),
            found.clone(),
            version,
        );
    }

    let lib = read_text(root, "crates/ridlc/src/lib.rs");
    check(
        "crates/ridlc/src/lib.rs (RIDL_RT_REQUIREMENT)",
        lib.as_deref()
            .and_then(|text| quoted_after(text, RIDL_RT_REQUIREMENT_PREFIX)),
        major_minor,
    );

    for manifest in RIDL_RT_MANIFESTS {
        let found = read_toml(root, manifest).and_then(|toml| {
            toml.get("dependencies")?
                .get("ridl-rt")?
                .get("version")?
                .as_str()
                .map(String::from)
        });
        check(&format!("{manifest} (ridl-rt)"), found, major_minor);
    }

    let cabin_manifest = read_text(root, "examples/cabin/Cargo.toml");
    check(
        "examples/cabin/Cargo.toml (the comment on the generated requirement)",
        cabin_manifest
            .as_deref()
            .and_then(|text| quoted_after(text, CABIN_COMMENT_PREFIX)),
        major_minor,
    );

    let deno_lock = read_json(root, "examples/cabin/web/deno.lock");
    // Every ridl-runtime link is checked, so a stale link beside a current one
    // is reported.
    let links: Vec<String> = deno_lock
        .as_ref()
        .and_then(|lock| lock.get("workspace")?.get("links")?.as_object())
        .map(|links| {
            links
                .keys()
                .filter_map(|key| key.strip_prefix(RIDL_RUNTIME_LINK_PREFIX))
                .map(String::from)
                .collect()
        })
        .unwrap_or_default();
    if links.is_empty() {
        check(
            "examples/cabin/web/deno.lock (ridl-runtime link)",
            None,
            version,
        );
    }
    for link in links {
        check(
            "examples/cabin/web/deno.lock (ridl-runtime link)",
            Some(link),
            version,
        );
    }

    let package = read_json(root, "editors/vscode/package.json");
    check(
        "editors/vscode/package.json (version)",
        package
            .as_ref()
            .and_then(|p| p.get("version")?.as_str().map(String::from)),
        version,
    );
    let package_lock = read_json(root, "editors/vscode/package-lock.json");
    check(
        "editors/vscode/package-lock.json (version)",
        package_lock
            .as_ref()
            .and_then(|p| p.get("version")?.as_str().map(String::from)),
        version,
    );
    check(
        "editors/vscode/package-lock.json (packages[\"\"].version)",
        package_lock.as_ref().and_then(|p| {
            p.get("packages")?
                .get("")?
                .get("version")?
                .as_str()
                .map(String::from)
        }),
        version,
    );

    errors
}

fn read_text(root: &Path, relative: &str) -> Option<String> {
    std::fs::read_to_string(root.join(relative)).ok()
}

fn read_toml(root: &Path, relative: &str) -> Option<toml::Table> {
    read_text(root, relative)?.parse().ok()
}

fn read_json(root: &Path, relative: &str) -> Option<serde_json::Value> {
    serde_json::from_str(&read_text(root, relative)?).ok()
}

fn lock_packages(lock: Option<&toml::Table>) -> Vec<toml::Table> {
    lock.and_then(|lock| lock.get("package")?.as_array().cloned())
        .unwrap_or_default()
        .into_iter()
        .filter_map(|package| package.as_table().cloned())
        .collect()
}

/// The text between the first occurrence of `prefix` and the next `"`.
fn quoted_after(text: &str, prefix: &str) -> Option<String> {
    let start = text.find(prefix)? + prefix.len();
    let rest = &text[start..];
    rest.find('"').map(|end| rest[..end].to_string())
}

#[test]
fn derived_pins_follow_the_workspace_version() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let manifest = read_toml(root, "Cargo.toml").expect("the root Cargo.toml must parse");
    let version = manifest
        .get("workspace")
        .and_then(|w| w.get("package"))
        .and_then(|p| p.get("version"))
        .and_then(|v| v.as_str())
        .expect("the root Cargo.toml must set [workspace.package] version");
    let found = derived_pin_errors(root, version);
    assert!(
        found.is_empty(),
        "these pins do not follow the workspace version {version}; \
         `just release-sync` updates them:\n  {}",
        found.join("\n  ")
    );
}

/// A synthetic tree with every pin at `version` and `major_minor`. Each test
/// takes its own directory, named after the test.
fn synthetic_tree(test: &str, version: &str, major_minor: &str) -> std::path::PathBuf {
    let dir =
        std::env::temp_dir().join(format!("xtask-derived-pins-{test}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let write = |relative: &str, text: String| {
        let path = dir.join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    };
    write(
        "Cargo.lock",
        format!(
            "version = 4\n\n[[package]]\nname = \"ridl-core\"\nversion = \"{version}\"\n\n\
             [[package]]\nname = \"ridl-ir\"\nversion = \"{version}\"\n\n\
             [[package]]\nname = \"serde\"\nversion = \"1.0.0\"\n\
             source = \"registry+https://github.com/rust-lang/crates.io-index\"\n"
        ),
    );
    write(
        "examples/cabin/Cargo.lock",
        CABIN_PATH_CRATES
            .iter()
            .map(|name| format!("[[package]]\nname = \"{name}\"\nversion = \"{version}\"\n\n"))
            .collect::<String>()
            // A registry `ridl-` package and a path package of the example
            // itself, which the check must both leave alone.
            + "[[package]]\nname = \"ridl-x\"\nversion = \"0.1.0\"\n\
               source = \"registry+https://github.com/rust-lang/crates.io-index\"\n\n\
               [[package]]\nname = \"cabin-app\"\nversion = \"0.0.0\"\n\n",
    );
    write(
        "crates/ridlc/src/lib.rs",
        format!("    {RIDL_RT_REQUIREMENT_PREFIX}{major_minor}\";\n"),
    );
    for manifest in RIDL_RT_MANIFESTS {
        write(
            manifest,
            format!(
                "[dependencies]\nridl-rt = {{ version = \"{major_minor}\", features = [\"std\"] }}\n"
            ),
        );
    }
    write(
        "examples/cabin/Cargo.toml",
        format!("# The generated manifest {CABIN_COMMENT_PREFIX}{major_minor}\"`, which\n"),
    );
    write(
        "examples/cabin/web/deno.lock",
        format!(
            "{{\"workspace\": {{\"links\": {{\"{RIDL_RUNTIME_LINK_PREFIX}{version}\": {{}}}}}}}}"
        ),
    );
    write(
        "editors/vscode/package.json",
        format!("{{\"name\": \"ridl-lang\", \"version\": \"{version}\"}}"),
    );
    write(
        "editors/vscode/package-lock.json",
        format!(
            "{{\"name\": \"ridl-lang\", \"version\": \"{version}\", \
             \"packages\": {{\"\": {{\"version\": \"{version}\"}}}}}}"
        ),
    );
    dir
}

fn replace_in(dir: &Path, relative: &str, from: &str, to: &str) {
    let path = dir.join(relative);
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains(from), "{relative} must contain {from}");
    std::fs::write(path, text.replace(from, to)).unwrap();
}

#[test]
fn all_pins_current_gives_no_error() {
    let dir = synthetic_tree("current", "0.15.0", "0.15");
    assert_eq!(derived_pin_errors(&dir, "0.15.0"), Vec::<String>::new());
}

/// Each pin, made stale on its own, gives exactly one error that names it.
/// Every case is a (file, text, replacement, the error's label) row.
#[test]
fn each_stale_pin_is_reported_alone() {
    let mut cases: Vec<(String, String, String, String)> = vec![(
        "Cargo.lock".into(),
        "name = \"ridl-core\"\nversion = \"0.15.0\"".into(),
        "name = \"ridl-core\"\nversion = \"0.14.0\"".into(),
        "Cargo.lock (package ridl-core)".into(),
    )];
    // The second workspace package, because a `version_files` entry rewrites
    // only the first match.
    cases.push((
        "Cargo.lock".into(),
        "name = \"ridl-ir\"\nversion = \"0.15.0\"".into(),
        "name = \"ridl-ir\"\nversion = \"0.14.0\"".into(),
        "Cargo.lock (package ridl-ir)".into(),
    ));
    for name in CABIN_PATH_CRATES {
        cases.push((
            "examples/cabin/Cargo.lock".into(),
            format!("name = \"{name}\"\nversion = \"0.15.0\""),
            format!("name = \"{name}\"\nversion = \"0.14.0\""),
            format!("examples/cabin/Cargo.lock (package {name})"),
        ));
    }
    for manifest in RIDL_RT_MANIFESTS {
        cases.push((
            manifest.into(),
            "\"0.15\"".into(),
            "\"0.14\"".into(),
            format!("{manifest} (ridl-rt)"),
        ));
    }
    for (file, from, to, label) in [
        (
            "crates/ridlc/src/lib.rs",
            "\"0.15\"",
            "\"0.14\"",
            "crates/ridlc/src/lib.rs",
        ),
        (
            "examples/cabin/Cargo.toml",
            "\"0.15\"",
            "\"0.14\"",
            "examples/cabin/Cargo.toml",
        ),
        (
            "examples/cabin/web/deno.lock",
            "@0.15.0",
            "@0.14.0",
            "examples/cabin/web/deno.lock",
        ),
        (
            "editors/vscode/package.json",
            "\"0.15.0\"",
            "\"0.14.0\"",
            "editors/vscode/package.json (version)",
        ),
        (
            "editors/vscode/package-lock.json",
            "\"ridl-lang\", \"version\": \"0.15.0\"",
            "\"ridl-lang\", \"version\": \"0.14.0\"",
            "editors/vscode/package-lock.json (version)",
        ),
        (
            "editors/vscode/package-lock.json",
            "{\"\": {\"version\": \"0.15.0\"",
            "{\"\": {\"version\": \"0.14.0\"",
            "editors/vscode/package-lock.json (packages[\"\"].version)",
        ),
    ] {
        cases.push((file.into(), from.into(), to.into(), label.into()));
    }
    for (index, (file, from, to, label)) in cases.iter().enumerate() {
        let dir = synthetic_tree(&format!("stale-{index}"), "0.15.0", "0.15");
        replace_in(&dir, file, from, to);
        let errors = derived_pin_errors(&dir, "0.15.0");
        assert_eq!(errors.len(), 1, "{label}: {errors:#?}");
        assert!(
            errors[0].starts_with(label.as_str()) && errors[0].contains("found 0.14"),
            "{label}: {}",
            errors[0]
        );
    }
}

/// Each pinned file, when it is missing, gives "not found" errors that name
/// it, and no other error.
#[test]
fn each_missing_file_is_reported() {
    let mut files = vec![
        "Cargo.lock",
        "examples/cabin/Cargo.lock",
        "crates/ridlc/src/lib.rs",
        "examples/cabin/Cargo.toml",
        "examples/cabin/web/deno.lock",
        "editors/vscode/package.json",
        "editors/vscode/package-lock.json",
    ];
    files.extend(RIDL_RT_MANIFESTS);
    for (index, file) in files.iter().enumerate() {
        let dir = synthetic_tree(&format!("missing-{index}"), "0.15.0", "0.15");
        std::fs::remove_file(dir.join(file)).unwrap();
        let errors = derived_pin_errors(&dir, "0.15.0");
        assert!(!errors.is_empty(), "{file}: no error");
        for error in &errors {
            assert!(
                error.starts_with(file) && error.contains("not found"),
                "{file}: {error}"
            );
        }
    }
}

#[test]
fn a_stale_ridl_runtime_link_beside_a_current_one_is_reported() {
    let dir = synthetic_tree("two-links", "0.15.0", "0.15");
    std::fs::write(
        dir.join("examples/cabin/web/deno.lock"),
        format!(
            "{{\"workspace\": {{\"links\": {{\
             \"{RIDL_RUNTIME_LINK_PREFIX}0.15.0\": {{}}, \
             \"{RIDL_RUNTIME_LINK_PREFIX}0.14.0\": {{}}}}}}}}"
        ),
    )
    .unwrap();
    let errors = derived_pin_errors(&dir, "0.15.0");
    assert_eq!(errors.len(), 1, "{errors:#?}");
    assert!(errors[0].contains("found 0.14.0"), "{}", errors[0]);
}

#[test]
fn pre_release_requirements_use_the_full_version() {
    let dir = synthetic_tree("prerelease", "0.15.0-rc.1", "0.15.0-rc.1");
    assert_eq!(
        derived_pin_errors(&dir, "0.15.0-rc.1"),
        Vec::<String>::new()
    );
    let dir = synthetic_tree("prerelease-mm", "0.15.0-rc.1", "0.15");
    let errors = derived_pin_errors(&dir, "0.15.0-rc.1");
    assert_eq!(errors.len(), 2 + RIDL_RT_MANIFESTS.len(), "{errors:#?}");
    assert!(
        errors
            .iter()
            .all(|e| e.contains("found 0.15, expected 0.15.0-rc.1")),
        "{errors:#?}"
    );
}

#[test]
fn a_full_version_where_the_major_minor_belongs_is_reported() {
    let dir = synthetic_tree("full-for-mm", "0.15.0", "0.15.0");
    let errors = derived_pin_errors(&dir, "0.15.0");
    assert_eq!(errors.len(), 2 + RIDL_RT_MANIFESTS.len(), "{errors:#?}");
    assert!(
        errors
            .iter()
            .all(|e| e.contains("found 0.15.0, expected 0.15")),
        "{errors:#?}"
    );
}

#[test]
fn a_listed_crate_missing_from_the_cabin_lock_is_reported() {
    let dir = synthetic_tree("listed-missing", "0.15.0", "0.15");
    replace_in(
        &dir,
        "examples/cabin/Cargo.lock",
        "[[package]]\nname = \"ridl-descriptor\"\nversion = \"0.15.0\"\n\n",
        "",
    );
    assert_eq!(
        derived_pin_errors(&dir, "0.15.0"),
        vec![
            "examples/cabin/Cargo.lock (package ridl-descriptor): \
             the pin was not found (expected 0.15.0)"
        ]
    );
}

#[test]
fn an_unlisted_ridl_path_package_in_the_cabin_lock_is_checked() {
    let dir = synthetic_tree("unlisted", "0.15.0", "0.15");
    let lock = dir.join("examples/cabin/Cargo.lock");
    let mut text = std::fs::read_to_string(&lock).unwrap();
    text.push_str("[[package]]\nname = \"ridl-new\"\nversion = \"0.14.0\"\n\n");
    std::fs::write(&lock, text).unwrap();
    assert_eq!(
        derived_pin_errors(&dir, "0.15.0"),
        vec!["examples/cabin/Cargo.lock (package ridl-new): found 0.14.0, expected 0.15.0"]
    );
}

#[test]
fn patch_release_keeps_the_major_minor_requirements() {
    let dir = synthetic_tree("patch", "0.14.1", "0.14");
    assert_eq!(derived_pin_errors(&dir, "0.14.1"), Vec::<String>::new());
}
