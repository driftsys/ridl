use ridl_cli::toolchain::*;
use ridl_core::release::{Release, parse_release};
use std::{
    collections::BTreeMap,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
};
use tempfile::TempDir;

const TARGET: &str = support::TARGET;
fn release(s: &str) -> Release {
    parse_release(s).unwrap()
}
fn binary_name() -> &'static str {
    if cfg!(windows) { "ridl.exe" } else { "ridl" }
}
fn installed(root: &Path, version: &str) -> PathBuf {
    let dir = root.join("versions").join(version);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join(binary_name()), b"installed binary").unwrap();
    fs::write(dir.join("LICENSE"), b"installed license").unwrap();
    fs::write(dir.join("THIRD-PARTY-NOTICES.txt"), b"installed notices").unwrap();
    fs::write(dir.join(".complete"), b"").unwrap();
    dir.join(binary_name()).canonicalize().unwrap()
}
fn directory_bytes(path: &Path) -> BTreeMap<OsString, Vec<u8>> {
    fs::read_dir(path)
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            (entry.file_name(), fs::read(entry.path()).unwrap())
        })
        .collect()
}
fn manifest(dir: &Path, text: &str) {
    fs::write(dir.join("ridl.toml"), text).unwrap();
}
fn pin(dir: &Path, value: &str) {
    manifest(
        dir,
        &format!("[package]\nname = 'fixture'\nridl-version = '{value}'\n"),
    );
}
fn running() -> PathBuf {
    std::env::current_exe().unwrap().canonicalize().unwrap()
}

#[test]
fn toolchain_roots_follow_platform_precedence() {
    let home = TempDir::new().unwrap();
    let home = home.path();
    let mut env = BTreeMap::new();
    assert_eq!(
        toolchain_root("linux", &env, Some(home)).unwrap(),
        home.join(".local/share/ridl")
    );
    assert_eq!(
        toolchain_root("macos", &env, Some(home)).unwrap(),
        home.join("Library/Application Support/ridl")
    );
    env.insert("XDG_DATA_HOME".into(), home.join("data").into_os_string());
    assert_eq!(
        toolchain_root("linux", &env, None).unwrap(),
        home.join("data/ridl")
    );
    env.insert(
        "LOCALAPPDATA".into(),
        home.join("local data").into_os_string(),
    );
    assert_eq!(
        toolchain_root("windows", &env, None).unwrap(),
        home.join("local data/ridl")
    );
    env.insert(
        "RIDL_TOOLCHAIN_DIR".into(),
        home.join("override with spaces").into_os_string(),
    );
    for platform in ["linux", "macos", "windows"] {
        assert_eq!(
            toolchain_root(platform, &env, None).unwrap(),
            home.join("override with spaces")
        );
    }
    env.insert(
        "RIDL_TOOLCHAIN_DIR".into(),
        OsString::from("relative tools"),
    );
    for platform in ["linux", "macos", "windows"] {
        let error = toolchain_root(platform, &env, None).unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
        assert!(
            error
                .to_string()
                .contains("RIDL_TOOLCHAIN_DIR must be an absolute path"),
            "{error}"
        );
    }
    env.insert("RIDL_TOOLCHAIN_DIR".into(), OsString::new());
    env.insert("XDG_DATA_HOME".into(), OsString::new());
    env.insert("LOCALAPPDATA".into(), OsString::new());
    for (platform, name) in [
        ("linux", "home"),
        ("macos", "home"),
        ("windows", "LOCALAPPDATA"),
    ] {
        let error = toolchain_root(platform, &env, None)
            .unwrap_err()
            .to_string();
        assert!(error.contains(name), "{error}");
    }
}

#[test]
fn toolchain_roots_windows_names_are_case_insensitive() {
    let home = TempDir::new().unwrap();
    let mut env = BTreeMap::from([(
        "LocalAppData".into(),
        home.path().join("data").into_os_string(),
    )]);
    let result = toolchain_root("windows", &env, None);
    assert!(
        result.is_ok(),
        "mixed-case LocalAppData lookup failed: {result:?}"
    );
    assert_eq!(result.unwrap(), home.path().join("data/ridl"));
    env.insert(
        "Ridl_Toolchain_Dir".into(),
        home.path().join("override").into_os_string(),
    );
    assert_eq!(
        toolchain_root("windows", &env, None).unwrap(),
        home.path().join("override")
    );
    // Unix environment names retain their case-sensitive semantics.
    assert_eq!(
        toolchain_root("linux", &env, Some(home.path())).unwrap(),
        home.path().join(".local/share/ridl")
    );
    env.insert("Ridl_Toolchain_Dir".into(), OsString::new());
    assert_eq!(
        toolchain_root("windows", &env, None).unwrap(),
        home.path().join("data/ridl")
    );
}

#[test]
fn toolchain_which_selects_highest_compatible_including_running() {
    let t = TempDir::new().unwrap();
    let workspace = TempDir::new().unwrap();
    pin(workspace.path(), "0.23.2");
    for v in ["0.23.0", "0.23.4", "0.23.10", "0.24.0"] {
        installed(t.path(), v);
    }
    let expected = t
        .path()
        .join("versions/0.23.10")
        .join(binary_name())
        .canonicalize()
        .unwrap();
    let select = |v, target| which(workspace.path(), t.path(), release(v), &running(), target);
    let selected = select("0.23.2", TARGET).unwrap();
    assert_eq!(selected.pin.as_deref(), Some("0.23.2"));
    assert_eq!(selected.path.as_deref(), Some(expected.as_path()));
    assert_eq!(selected.source, Some(WhichSource::Installed));
    for v in ["0.23.10", "0.23.12"] {
        let selected = select(v, TARGET).unwrap();
        assert_eq!(selected.path, Some(running()));
        assert_eq!(selected.source, Some(WhichSource::Running));
    }
    installed(t.path(), "0.23.99");
    fs::remove_file(t.path().join("versions/0.23.99").join(binary_name())).unwrap();
    assert_eq!(select("0.23.2", TARGET).unwrap().path, Some(expected));
    let unsupported = if cfg!(windows) {
        "x86_64-pc-windows-gnu"
    } else {
        "x86_64-unknown-linux-musl"
    };
    assert!(
        select("0.22.0", unsupported)
            .unwrap_err()
            .to_string()
            .contains(unsupported)
    );
    assert_eq!(
        select("0.23.2", unsupported).unwrap().source,
        Some(WhichSource::Running)
    );
    let empty = TempDir::new().unwrap();
    assert_eq!(
        which(
            workspace.path(),
            empty.path(),
            release("0.23.2"),
            &running(),
            "unsupported"
        )
        .unwrap()
        .source,
        Some(WhichSource::Running)
    );
    // A broken versions path is an input error, never an empty inventory.
    fs::write(empty.path().join("versions"), b"not a directory").unwrap();
    assert!(
        which(
            workspace.path(),
            empty.path(),
            release("0.22.0"),
            &running(),
            TARGET
        )
        .is_err()
    );
}

#[test]
fn toolchain_which_schema_one_covers_all_statuses() {
    let t = TempDir::new().unwrap();
    let w = TempDir::new().unwrap();
    let query = || {
        which(
            w.path(),
            &t.path().join("absent"),
            release("0.23.2"),
            &running(),
            TARGET,
        )
        .unwrap()
    };
    for (content, status, code, pin_value, has_manifest) in [
        (None, "unpinned", 0, None, false),
        (Some("[package]\nname='x'\n"), "unpinned", 0, None, true),
        (
            Some("[workspace]\nridl-version='0.24.0'\n"),
            "missing",
            1,
            Some("0.24.0"),
            true,
        ),
        (
            Some("[workspace]\nridl-version='bad'\n"),
            "invalid",
            1,
            None,
            true,
        ),
        (
            Some("[package]\nridl-version=7\n"),
            "invalid",
            1,
            None,
            true,
        ),
        (
            Some("[package]\nridl-version='0.22.1'\n"),
            "invalid",
            1,
            Some("0.22.1"),
            true,
        ),
        (
            Some("[workspace]\nridl-version='0.23.0'\n"),
            "compatible",
            0,
            Some("0.23.0"),
            true,
        ),
    ] {
        if let Some(s) = content {
            manifest(w.path(), s);
        }
        let r = query();
        let json = serde_json::to_value(&r).unwrap();
        let mut keys: Vec<_> = json
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            [
                "diagnostic",
                "manifest",
                "path",
                "pin",
                "running",
                "schema",
                "source",
                "status"
            ]
        );
        assert_eq!(json["schema"], 1);
        assert_eq!(json["running"], "0.23.2");
        assert_eq!(json["status"], status);
        assert_eq!(r.exit_code(), code);
        assert_eq!(r.pin.as_deref(), pin_value);
        assert_eq!(
            r.manifest,
            has_manifest.then(|| w.path().join("ridl.toml").canonicalize().unwrap())
        );
        if matches!(status, "compatible" | "unpinned") {
            assert_eq!(r.path, Some(running()));
            assert_eq!(json["source"], "running");
        } else {
            assert!(json["path"].is_null());
            assert!(json["source"].is_null());
        }
        if status == "invalid" {
            assert_eq!(
                json["diagnostic"]["code"],
                if pin_value.is_some() {
                    "MANI-025"
                } else {
                    "MANI-024"
                }
            );
            assert!(!json["diagnostic"]["message"].as_str().unwrap().is_empty());
        } else {
            assert!(json["diagnostic"].is_null());
        }
    }
    for text in ["not = [", "workspace=7", "package='wrong'"] {
        manifest(w.path(), text);
        assert!(
            which(w.path(), t.path(), release("0.23.2"), &running(), TARGET).is_err(),
            "{text}"
        );
    }
    assert!(
        which(
            &w.path().join("missing"),
            t.path(),
            release("0.23.2"),
            &running(),
            TARGET
        )
        .is_err()
    );
}

#[test]
fn toolchain_list_sorts_numerically_and_marks_damage() {
    let t = TempDir::new().unwrap();
    assert!(list(t.path()).unwrap().is_empty());
    for v in ["0.24.0", "0.23.10", "0.23.4", "0.23.0"] {
        installed(t.path(), v);
    }
    let versions = t.path().join("versions");
    for name in [".tmp-0.23.0-1", "unrelated", "00.23.0"] {
        fs::create_dir(versions.join(name)).unwrap();
    }
    fs::write(versions.join(".0.23.0.lock"), b"").unwrap();
    fs::write(versions.join("0.23.90"), b"not a directory").unwrap();
    fs::remove_file(versions.join("0.23.4/.complete")).unwrap();
    fs::create_dir(versions.join("0.23.4/.complete")).unwrap();
    fs::remove_file(versions.join("0.23.10").join(binary_name())).unwrap();
    let items = list(t.path()).unwrap();
    assert_eq!(
        items
            .iter()
            .map(|i| i.release.to_string())
            .collect::<Vec<_>>(),
        ["0.23.0", "0.23.4", "0.23.10", "0.24.0"]
    );
    assert!(items[0].complete && !items[0].damaged);
    assert!(!items[1].complete && items[1].damaged);
    assert!(items[2].complete && items[2].damaged);
    fs::remove_dir(versions.join("0.23.4/.complete")).unwrap();
    fs::write(versions.join("0.23.4/.complete"), b"not empty").unwrap();
    assert!(list(t.path()).unwrap()[1].damaged);
    let broken = TempDir::new().unwrap();
    fs::write(broken.path().join("versions"), b"file").unwrap();
    assert!(list(broken.path()).is_err());
}

#[test]
fn toolchain_target_rejects_unpublished_triples() {
    for target in [
        "x86_64-unknown-linux-gnu",
        "aarch64-unknown-linux-gnu",
        "x86_64-apple-darwin",
        "aarch64-apple-darwin",
        "x86_64-pc-windows-msvc",
    ] {
        assert_eq!(release_target(target).unwrap(), target);
    }
    assert!(
        release_target("x86_64-unknown-linux-musl")
            .unwrap_err()
            .to_string()
            .contains("x86_64-unknown-linux-musl")
    );
}

#[allow(dead_code)]
mod toolchain_support;
use toolchain_support as support;

#[test]
fn test_signer_produces_minisign_signatures() {
    use support::{other_test_public_key, sign, test_public_key};
    assert_ne!(test_public_key(), other_test_public_key());
    let key = minisign_verify::PublicKey::from_base64(&test_public_key()).unwrap();
    let sig = minisign_verify::Signature::decode(&sign(b"payload", "ridl 0.23.0 t")).unwrap();
    assert!(key.verify(b"payload", &sig, false).is_ok());
    assert_eq!(sig.trusted_comment(), "ridl 0.23.0 t");
}

#[test]
fn toolchain_install_requires_a_valid_signature() {
    let expected = format!("ridl 0.23.0 {TARGET}");
    for case in [
        "wrong-key",
        "old-release",
        "other-target",
        "prefix",
        "missing",
        "tampered",
        "oversize",
    ] {
        let assets = TempDir::new().unwrap();
        let root = TempDir::new().unwrap();
        let dir = support::fixture(assets.path(), &support::regular_files());
        let archive_path = dir.join(support::archive_name());
        let minisig = dir.join(format!("{}.minisig", support::archive_name()));
        let archive = fs::read(&archive_path).unwrap();
        let mut key = support::test_public_key();
        let (want, comment): (&str, Option<String>) = match case {
            "wrong-key" => {
                key = support::other_test_public_key();
                ("signature", None)
            }
            "old-release" => (&expected, Some(format!("ridl 0.22.9 {TARGET}"))),
            "other-target" => (
                &expected,
                Some("ridl 0.23.0 x86_64-pc-windows-msvc".to_string()),
            ),
            "prefix" => (&expected, Some(format!("{expected} extra"))),
            "missing" => {
                fs::remove_file(&minisig).unwrap();
                (".minisig", None)
            }
            "tampered" => {
                support::write_unsigned_asset(&dir, b"other archive bytes");
                ("signature", None)
            }
            "oversize" => {
                fs::write(&minisig, vec![b'a'; 4097]).unwrap();
                ("exceeds 4096", None)
            }
            _ => unreachable!(),
        };
        if let Some(comment) = comment {
            fs::write(&minisig, support::sign(&archive, &comment)).unwrap();
        }
        let error = install(
            root.path(),
            release("0.23.0"),
            TARGET,
            &support::source(assets.path()),
            &key,
        )
        .unwrap_err();
        assert!(error.to_string().contains(want), "{case}: {error}");
        assert!(!root.path().join("versions/0.23.0").exists(), "{case}");
        support::assert_no_stage(root.path());
    }
}

#[test]
fn toolchain_install_refuses_an_undecodable_public_key_before_download() {
    let root = TempDir::new().unwrap();
    let error = install(
        root.path(),
        release("0.23.0"),
        TARGET,
        "file:///nonexistent-origin",
        "PLACEHOLDER",
    )
    .unwrap_err();
    assert!(error.to_string().contains("release public key"), "{error}");
    assert!(!error.to_string().contains("nonexistent-origin"), "{error}");
}

#[test]
fn toolchain_install_verifies_and_publishes_complete_release() {
    let root = TempDir::new().unwrap();
    let assets = TempDir::new().unwrap();
    support::fixture(assets.path(), &support::regular_files());
    let p = install(
        root.path(),
        release("0.23.0"),
        support::TARGET,
        &support::source(assets.path()),
        &support::test_public_key(),
    )
    .unwrap();
    assert_eq!(
        p,
        root.path()
            .canonicalize()
            .unwrap()
            .join("versions/0.23.0")
            .join(binary_name())
    );
    support::assert_complete(&p);
    support::assert_no_stage(root.path());
    fs::remove_dir_all(assets.path()).unwrap();
    assert_eq!(
        install(
            root.path(),
            release("0.23.0"),
            support::TARGET,
            &support::source(assets.path()),
            &support::test_public_key()
        )
        .unwrap(),
        p
    );
    assert!(
        install(
            root.path(),
            release("0.22.9"),
            support::TARGET,
            &support::source(assets.path()),
            &support::test_public_key()
        )
        .is_err()
    );
}

#[test]
fn toolchain_install_and_which_use_the_target_binary_name() {
    for (target, binary) in [
        ("x86_64-pc-windows-msvc", "ridl.exe"),
        ("aarch64-apple-darwin", "ridl"),
        ("x86_64-apple-darwin", "ridl"),
        ("aarch64-unknown-linux-gnu", "ridl"),
        ("x86_64-unknown-linux-gnu", "ridl"),
    ] {
        let root = TempDir::new().unwrap();
        let workspace = TempDir::new().unwrap();
        pin(workspace.path(), "0.23.0");
        let assets = TempDir::new().unwrap();
        support::fixture_for(
            assets.path(),
            "0.23.0",
            target,
            &[
                (binary, b'0', support::BINARY),
                ("LICENSE", b'0', support::LICENSE),
                ("THIRD-PARTY-NOTICES.txt", b'0', support::NOTICES),
            ],
        );
        let result = install(
            root.path(),
            release("0.23.0"),
            target,
            &support::source(assets.path()),
            &support::test_public_key(),
        )
        .unwrap();
        assert_eq!(result.file_name().unwrap(), binary);
        let selected = which(
            workspace.path(),
            root.path(),
            release("0.22.0"),
            &running(),
            target,
        )
        .unwrap();
        assert_eq!(selected.status, WhichStatus::Compatible, "{target}");
        assert_eq!(selected.path, Some(result));
        assert_eq!(selected.source, Some(WhichSource::Installed));
        let unsupported = if binary == "ridl.exe" {
            "x86_64-pc-windows-gnu"
        } else {
            "x86_64-unknown-linux-musl"
        };
        let refused = which(
            workspace.path(),
            root.path(),
            release("0.22.0"),
            &running(),
            unsupported,
        );
        assert!(
            refused.is_err(),
            "unsupported target was not refused: {unsupported}"
        );
        assert!(refused.unwrap_err().to_string().contains(unsupported));
        if target == env!("RIDL_BUILD_TARGET") {
            let inventory = list(root.path()).unwrap();
            assert_eq!(inventory.len(), 1);
            assert!(!inventory[0].damaged);
        }
    }
}

#[test]
fn toolchain_install_rejects_checksum_and_unsafe_archive() {
    let root = TempDir::new().unwrap();
    let assets = TempDir::new().unwrap();
    let versions = root.path().join("versions");
    fs::create_dir(&versions).unwrap();
    let sentinel = versions.join("outside");
    fs::write(&sentinel, b"unchanged").unwrap();
    let reject = || {
        assert!(
            install(
                root.path(),
                release("0.23.0"),
                support::TARGET,
                &support::source(assets.path()),
                &support::test_public_key()
            )
            .is_err()
        );
        assert!(!root.path().join("versions/0.23.0").exists());
        support::assert_no_stage(root.path());
        assert_eq!(fs::read(&sentinel).unwrap(), b"unchanged");
        let entries: Vec<_> = fs::read_dir(&versions)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .filter(|n| n != ".0.23.0.lock")
            .collect();
        assert_eq!(
            entries,
            [OsString::from("outside")],
            "archive escaped staging"
        );
    };
    let dir = support::fixture(assets.path(), &support::regular_files());
    let sha = dir.join(format!("{}.sha256", support::archive_name()));
    use sha2::{Digest, Sha256};
    let correct_hash = format!(
        "{:x}",
        Sha256::digest(fs::read(dir.join(support::archive_name())).unwrap())
    );
    for record in [
        "bad checksum".to_string(),
        format!("{correct_hash}  wrong.tar.gz\n"),
        format!("{}  {}\n", "0".repeat(64), support::archive_name()),
    ] {
        fs::write(&sha, record).unwrap();
        reject();
    }
    support::write_asset(&dir, b"corrupt gzip");
    reject();
    support::fixture(assets.path(), &[("LICENSE", b'0', support::LICENSE)]);
    reject();
    for (name, kind) in [
        (".complete", b'0'),
        ("../outside", b'0'),
        ("../escaped", b'0'),
        ("/absolute", b'0'),
        ("nested/ridl", b'0'),
        ("LICENSE", b'0'),
        ("unknown", b'0'),
    ] {
        let mut entries = support::regular_files();
        entries.push((name, kind, b"unsafe"));
        support::fixture(assets.path(), &entries);
        reject();
    }
    for kind in *b"12" {
        let mut entries = support::regular_files();
        entries.retain(|(name, _, _)| *name != binary_name());
        entries.push((binary_name(), kind, b""));
        support::fixture(assets.path(), &entries);
        reject();
    }
    // Even an empty completion marker must be created by the installer itself.
    let mut entries = support::regular_files();
    entries.push((".complete", b'0', b""));
    support::fixture(assets.path(), &entries);
    reject();
}

#[test]
fn toolchain_install_rejects_absolute_archive_path() {
    let root = TempDir::new().unwrap();
    let assets = TempDir::new().unwrap();
    let outside = TempDir::new().unwrap();
    let escaped = outside.path().join("escaped");
    let escaped_name = escaped.to_str().unwrap();
    let mut entries = support::regular_files();
    entries.push((escaped_name, b'0', b"unsafe"));
    support::fixture(assets.path(), &entries);
    let result = install(
        root.path(),
        release("0.23.0"),
        support::TARGET,
        &support::source(assets.path()),
        &support::test_public_key(),
    );
    assert!(result.is_err());
    assert!(!escaped.exists(), "absolute archive path escaped staging");
    assert!(fs::read_dir(outside.path()).unwrap().next().is_none());
    assert!(!root.path().join("versions/0.23.0").exists());
    support::assert_no_stage(root.path());
    assert_eq!(
        fs::read_dir(root.path().join("versions")).unwrap().count(),
        1
    );
}

#[test]
fn toolchain_install_missing_asset_cleans_only_its_stage() {
    let root = TempDir::new().unwrap();
    let assets = TempDir::new().unwrap();
    let other = installed(root.path(), "0.24.0");
    let before = directory_bytes(other.parent().unwrap());
    assert!(
        install(
            root.path(),
            release("0.23.0"),
            support::TARGET,
            &support::source(assets.path()),
            &support::test_public_key()
        )
        .is_err()
    );
    support::assert_no_stage(root.path());
    assert_eq!(directory_bytes(other.parent().unwrap()), before);
    let dir = support::fixture(assets.path(), &support::regular_files());
    fs::remove_file(dir.join(support::archive_name())).unwrap();
    assert!(
        install(
            root.path(),
            release("0.23.0"),
            support::TARGET,
            &support::source(assets.path()),
            &support::test_public_key()
        )
        .is_err()
    );
    support::assert_no_stage(root.path());
}

#[test]
fn toolchain_install_removes_only_stale_cli_stages_under_lock() {
    let root = TempDir::new().unwrap();
    let assets = TempDir::new().unwrap();
    support::fixture(assets.path(), &support::regular_files());
    let versions = root.path().join("versions");
    fs::create_dir(&versions).unwrap();
    let stale_names = [
        format!(".tmp-0.23.0-{}", std::process::id()),
        ".tmp-0.23.0-123456".into(),
    ];
    for name in &stale_names {
        fs::create_dir(versions.join(name)).unwrap();
        fs::write(versions.join(name).join("incomplete"), b"stale bytes").unwrap();
    }
    let preserved_names = [
        ".tmp-0.24.0-1",
        ".tmp-0.23.00-1",
        ".tmp-script-0.23.0-123456",
        ".tmp-0.23.0-killed",
        ".tmp-0.23.0-",
        ".tmp-0.23.0-12x",
    ];
    for name in preserved_names {
        fs::create_dir(versions.join(name)).unwrap();
        fs::write(
            versions.join(name).join("preserve"),
            b"live or unrelated stage",
        )
        .unwrap();
    }
    let result = install(
        root.path(),
        release("0.23.0"),
        support::TARGET,
        &support::source(assets.path()),
        &support::test_public_key(),
    );
    assert!(
        result.is_ok(),
        "stale stage prevented installation: {result:?}"
    );
    support::assert_complete(&result.unwrap());
    for name in &stale_names {
        assert!(!versions.join(name).exists(), "stale stage remains: {name}");
    }
    for name in preserved_names {
        assert_eq!(
            fs::read(versions.join(name).join("preserve"))
                .unwrap_or_else(|error| panic!("removed unrelated stage {name}: {error}")),
            b"live or unrelated stage"
        );
    }
}

#[test]
fn toolchain_install_limits_archive_size() {
    assert_download_size_limit(false, 256 * 1024 * 1024);
}

#[test]
fn toolchain_install_limits_checksum_size() {
    assert_download_size_limit(true, 4 * 1024);
}

fn assert_download_size_limit(checksum: bool, limit: u64) {
    let root = TempDir::new().unwrap();
    let assets = TempDir::new().unwrap();
    let dir = support::fixture(assets.path(), &support::regular_files());
    let name = if checksum {
        format!("{}.sha256", support::archive_name())
    } else {
        support::archive_name()
    };
    let file = fs::OpenOptions::new()
        .write(true)
        .open(dir.join(&name))
        .unwrap();
    file.set_len(limit + 1).unwrap();
    let result = install(
        root.path(),
        release("0.23.0"),
        support::TARGET,
        &support::source(assets.path()),
        &support::test_public_key(),
    );
    assert!(result.is_err(), "oversized download accepted: {name}");
    let error = result.unwrap_err().to_string();
    assert!(
        error.contains("exceeds") && error.contains(&limit.to_string()),
        "wrong size refusal: {error}"
    );
    assert!(!root.path().join("versions/0.23.0").exists());
    support::assert_no_stage(root.path());
    assert_eq!(
        fs::read_dir(root.path().join("versions")).unwrap().count(),
        1
    );
}

// Child processes exercise OS file locking, rather than thread-local serialization.
#[test]
fn toolchain_install_child() {
    let Some(root) = std::env::var_os("RIDL_TEST_INSTALL_ROOT") else {
        return;
    };
    fs::write(std::env::var_os("RIDL_TEST_READY").unwrap(), b"ready").unwrap();
    let path = install(
        Path::new(&root),
        release("0.23.0"),
        support::TARGET,
        &std::env::var("RIDL_TEST_SOURCE").unwrap(),
        // The child process generated its own key pair; the parent's key signed the fixture.
        &std::env::var("RIDL_TEST_PUBLIC_KEY").unwrap(),
    )
    .unwrap();
    fs::write(
        std::env::var_os("RIDL_TEST_RESULT").unwrap(),
        path.to_str().unwrap(),
    )
    .unwrap();
}
struct Child(std::process::Child);
impl Drop for Child {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn child(root: &Path, source: &str, ready: &Path, result: &Path) -> Child {
    Child(
        std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "toolchain_install_child", "--nocapture"])
            .env("RIDL_TEST_INSTALL_ROOT", root)
            .env("RIDL_TEST_SOURCE", source)
            .env("RIDL_TEST_PUBLIC_KEY", support::test_public_key())
            .env("RIDL_TEST_READY", ready)
            .env("RIDL_TEST_RESULT", result)
            .stdout(std::process::Stdio::null())
            .spawn()
            .unwrap(),
    )
}
fn wait_until(mut predicate: impl FnMut() -> bool) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !predicate() {
        assert!(std::time::Instant::now() < deadline, "child timed out");
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}
fn finish(child: &mut Child) {
    wait_until(|| child.0.try_wait().unwrap().is_some());
    assert!(child.0.wait().unwrap().success());
}

#[test]
fn toolchain_install_reuses_a_complete_entry_only_under_the_lock() {
    let root = TempDir::new().unwrap();
    let assets = TempDir::new().unwrap();
    support::fixture(assets.path(), &support::regular_files());
    installed(root.path(), "0.23.0");
    let versions = root.path().join("versions");
    let directory = versions.canonicalize().unwrap().join("0.23.0");
    // An uninstall-like holder owns the lock while the entry is still complete.
    let lock = fs::File::create(versions.join(".0.23.0.lock")).unwrap();
    lock.lock().unwrap();
    let ready = root.path().join("ready");
    let result = root.path().join("result");
    let mut c = child(
        root.path(),
        &support::source(assets.path()),
        &ready,
        &result,
    );
    wait_until(|| ready.exists());
    std::thread::sleep(std::time::Duration::from_millis(500));
    assert!(
        c.0.try_wait().unwrap().is_none() && !result.exists(),
        "install reported a complete entry without holding the lock"
    );
    fs::remove_dir_all(&directory).unwrap();
    lock.unlock().unwrap();
    finish(&mut c);
    let reported = PathBuf::from(fs::read_to_string(&result).unwrap());
    assert_eq!(reported, directory.join(binary_name()));
    support::assert_complete(&reported);
    assert_ne!(fs::read(&reported).unwrap(), b"installed binary");
}

#[test]
fn toolchain_install_waits_rechecks_and_preserves_damage() {
    let root = TempDir::new().unwrap();
    let assets = TempDir::new().unwrap();
    let versions = root.path().join("versions");
    fs::create_dir(&versions).unwrap();
    let lock = fs::File::create(versions.join(".0.23.0.lock")).unwrap();
    lock.lock().unwrap();
    let ready = root.path().join("ready");
    let result = root.path().join("result");
    let mut c = child(
        root.path(),
        &support::source(assets.path()),
        &ready,
        &result,
    );
    wait_until(|| ready.exists());
    std::thread::sleep(std::time::Duration::from_millis(200));
    assert!(
        c.0.try_wait().unwrap().is_none(),
        "installer did not wait for the lock"
    );
    let expected = installed(root.path(), "0.23.0");
    fs::remove_dir_all(assets.path()).unwrap();
    lock.unlock().unwrap();
    finish(&mut c);
    assert_eq!(
        fs::read_to_string(&result).unwrap(),
        expected.to_str().unwrap()
    );
    fs::remove_file(expected.parent().unwrap().join(".complete")).unwrap();
    let before = directory_bytes(expected.parent().unwrap());
    let error = install(
        root.path(),
        release("0.23.0"),
        support::TARGET,
        "file:///missing",
        &support::test_public_key(),
    )
    .unwrap_err()
    .to_string();
    assert!(
        error.contains(expected.parent().unwrap().to_str().unwrap()),
        "{error}"
    );
    assert_eq!(directory_bytes(expected.parent().unwrap()), before);
    assert!(!expected.parent().unwrap().join(".complete").exists());
    support::assert_no_stage(root.path());
    let root = TempDir::new().unwrap();
    let assets = TempDir::new().unwrap();
    support::fixture(assets.path(), &support::regular_files());
    let mut a = child(
        root.path(),
        &support::source(assets.path()),
        &root.path().join("ready-a"),
        &root.path().join("result-a"),
    );
    let mut b = child(
        root.path(),
        &support::source(assets.path()),
        &root.path().join("ready-b"),
        &root.path().join("result-b"),
    );
    finish(&mut a);
    finish(&mut b);
    assert_eq!(
        fs::read(root.path().join("result-a")).unwrap(),
        fs::read(root.path().join("result-b")).unwrap()
    );
    let p = PathBuf::from(fs::read_to_string(root.path().join("result-a")).unwrap());
    support::assert_complete(&p);
    support::assert_no_stage(root.path());
}

fn cli(cwd: &Path, root: &Path, args: &[&str]) -> std::process::Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_ridl"))
        .current_dir(cwd)
        .env("RIDL_TOOLCHAIN_DIR", root)
        .env("RIDL_TEST_RUNNING_RELEASE", "0.23.2")
        .args(args)
        .output()
        .unwrap()
}
fn output(output: &std::process::Output, code: i32, stdout: &str, has_error: bool) {
    assert_eq!(
        output.status.code(),
        Some(code),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), stdout);
    assert_eq!(
        !output.stderr.is_empty(),
        has_error,
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn toolchain_cli_refuses_a_relative_root() {
    let w = TempDir::new().unwrap();
    let env = BTreeMap::from([(
        "RIDL_TOOLCHAIN_DIR".to_string(),
        OsString::from("relative tools"),
    )]);
    let error = toolchain_root("linux", &env, None).unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    assert!(
        error
            .to_string()
            .contains("RIDL_TOOLCHAIN_DIR must be an absolute path")
    );
    let list = std::process::Command::new(env!("CARGO_BIN_EXE_ridl"))
        .current_dir(w.path())
        .env("RIDL_TOOLCHAIN_DIR", "relative tools")
        .args(["toolchain", "list"])
        .output()
        .unwrap();
    assert_eq!(list.status.code(), Some(2));
    assert!(list.stdout.is_empty());
    assert!(
        String::from_utf8_lossy(&list.stderr)
            .contains("RIDL_TOOLCHAIN_DIR must be an absolute path")
    );
    assert!(!w.path().join("relative tools").exists());
}

#[test]
fn toolchain_cli_streams_and_exit_codes() {
    let root = TempDir::new().unwrap();
    let w = TempDir::new().unwrap();
    output(
        &cli(w.path(), root.path(), &["toolchain", "list"]),
        0,
        "",
        false,
    );
    output(
        &cli(w.path(), root.path(), &["toolchain", "install"]),
        2,
        "",
        true,
    );
    let exe = Path::new(env!("CARGO_BIN_EXE_ridl"))
        .canonicalize()
        .unwrap();
    output(
        &cli(w.path(), root.path(), &["toolchain", "which"]),
        0,
        &format!("{}\n", exe.display()),
        false,
    );
    pin(w.path(), "0.24.0");
    let o = cli(w.path(), root.path(), &["toolchain", "which", "--json"]);
    assert_eq!(o.status.code(), Some(1));
    assert!(o.stderr.is_empty());
    let json: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(json["status"], "missing");
    let missing = cli(w.path(), root.path(), &["toolchain", "which"]);
    output(&missing, 1, "", true);
    assert_eq!(
        String::from_utf8_lossy(&missing.stderr),
        "No compatible ridl toolchain is installed; run `ridl toolchain install 0.24.0`.\n"
    );
    for version in ["bad", "v0.23.0", "0.23.0-rc.1", "0.22.0"] {
        output(
            &cli(w.path(), root.path(), &["toolchain", "install", version]),
            2,
            "",
            true,
        );
    }
    for text in [
        "[package]\nridl-version='bad'",
        "[workspace]\nridl-version='0.22.0'",
        "not=[",
        "workspace=7",
        "package='bad'",
    ] {
        manifest(w.path(), text);
        output(
            &cli(w.path(), root.path(), &["toolchain", "install"]),
            2,
            "",
            true,
        );
        if text.contains("ridl-version") {
            let o = cli(w.path(), root.path(), &["toolchain", "which", "--json"]);
            assert_eq!(o.status.code(), Some(1));
            assert!(o.stderr.is_empty());
            let value: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
            assert_eq!(value["status"], "invalid");
            let invalid = cli(w.path(), root.path(), &["toolchain", "which"]);
            output(&invalid, 1, "", true);
            let expected = if text.contains("bad") {
                "MANI-024: `ridl-version` must be a string of the form MAJOR.MINOR.PATCH\n"
                    .to_string()
            } else {
                format!(
                    "MANI-025: `ridl-version` 0.22.0 is below the minimum pinnable release {}\n",
                    ridl_core::release::MIN_PINNABLE_RELEASE
                )
            };
            assert_eq!(String::from_utf8_lossy(&invalid.stderr), expected);
        } else {
            output(
                &cli(w.path(), root.path(), &["toolchain", "which", "--json"]),
                2,
                "",
                true,
            );
        }
    }
    output(
        &cli(
            w.path(),
            root.path(),
            &["toolchain", "which", "missing", "--json"],
        ),
        2,
        "",
        true,
    );
    let expected = installed(root.path(), "0.23.0");
    pin(w.path(), "0.23.0");
    output(
        &cli(w.path(), root.path(), &["toolchain", "install"]),
        0,
        &format!("{}\n", expected.display()),
        false,
    );
    output(
        &cli(w.path(), root.path(), &["toolchain", "install", "0.23.0"]),
        0,
        &format!("{}\n", expected.display()),
        false,
    );
    let damaged = installed(root.path(), "0.23.4");
    fs::remove_file(damaged.parent().unwrap().join(".complete")).unwrap();
    let listing = cli(w.path(), root.path(), &["toolchain", "list"]);
    assert!(listing.status.success());
    assert!(listing.stderr.is_empty());
    let listing = String::from_utf8(listing.stdout).unwrap();
    assert_eq!(
        listing,
        format!(
            "0.23.0\t{}\n0.23.4\t{} (damaged)\n",
            expected.parent().unwrap().display(),
            damaged.parent().unwrap().display()
        )
    );
    output(
        &cli(
            w.path(),
            root.path(),
            &["toolchain", "install", "--base-url", "file:///missing"],
        ),
        2,
        "",
        true,
    );
    // Selection reads the root pin rather than a member's conflicting pin.
    let member = w.path().join("member");
    fs::create_dir(&member).unwrap();
    pin(&member, "0.24.0");
    manifest(
        w.path(),
        "[workspace]\nmembers=['member']\nridl-version='0.23.0'\n",
    );
    let selected = cli(&member, root.path(), &["toolchain", "which", "--json"]);
    assert!(selected.status.success());
    let selected: serde_json::Value = serde_json::from_slice(&selected.stdout).unwrap();
    assert_eq!(selected["pin"], "0.23.0");
    assert_eq!(
        selected["manifest"],
        w.path()
            .join("ridl.toml")
            .canonicalize()
            .unwrap()
            .to_str()
            .unwrap()
    );
}

#[cfg(unix)]
#[test]
fn toolchain_cli_unreadable_inputs_and_broken_stdout_exit_two() {
    use std::os::unix::fs::PermissionsExt;
    let root = TempDir::new().unwrap();
    let w = TempDir::new().unwrap();
    pin(w.path(), "0.23.0");
    let m = w.path().join("ridl.toml");
    fs::set_permissions(&m, fs::Permissions::from_mode(0o000)).unwrap();
    output(
        &cli(w.path(), root.path(), &["toolchain", "which", "--json"]),
        2,
        "",
        true,
    );
    output(
        &cli(w.path(), root.path(), &["toolchain", "install"]),
        2,
        "",
        true,
    );
    fs::set_permissions(&m, fs::Permissions::from_mode(0o600)).unwrap();
    installed(root.path(), "0.23.0");
    let (read, write) = std::io::pipe().unwrap();
    drop(read);
    let o = std::process::Command::new(env!("CARGO_BIN_EXE_ridl"))
        .current_dir(w.path())
        .env("RIDL_TOOLCHAIN_DIR", root.path())
        .args(["toolchain", "install", "0.23.0"])
        .stdout(write)
        .stderr(std::process::Stdio::piped())
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(2));
    let versions = root.path().join("versions");
    fs::set_permissions(&versions, fs::Permissions::from_mode(0o000)).unwrap();
    assert!(list(root.path()).is_err());
    fs::set_permissions(&versions, fs::Permissions::from_mode(0o700)).unwrap();
}

#[test]
fn toolchain_install_rechecks_the_requested_target_binary() {
    use sha2::{Digest, Sha256};
    let root = TempDir::new().unwrap();
    let assets = TempDir::new().unwrap();
    let (target, binary) = if cfg!(windows) {
        ("aarch64-apple-darwin", "ridl")
    } else {
        ("x86_64-pc-windows-msvc", "ridl.exe")
    };
    let entries = [
        (binary, b'0', support::BINARY),
        ("LICENSE", b'0', support::LICENSE),
        ("THIRD-PARTY-NOTICES.txt", b'0', support::NOTICES),
    ];
    let dir = support::fixture(assets.path(), &entries);
    let bytes = fs::read(dir.join(support::archive_name())).unwrap();
    let archive = format!("ridl-{target}.tar.gz");
    fs::write(dir.join(&archive), &bytes).unwrap();
    fs::write(
        dir.join(format!("{archive}.sha256")),
        format!("{:x}  {archive}\n", Sha256::digest(&bytes)),
    )
    .unwrap();
    fs::write(
        dir.join(format!("{archive}.minisig")),
        support::sign(&bytes, &format!("ridl 0.23.0 {target}")),
    )
    .unwrap();
    let first = install(
        root.path(),
        release("0.23.0"),
        target,
        &support::source(assets.path()),
        &support::test_public_key(),
    )
    .unwrap();
    support::assert_complete(&first);
    fs::remove_dir_all(assets.path()).unwrap();
    let repeated = install(
        root.path(),
        release("0.23.0"),
        target,
        &support::source(assets.path()),
        &support::test_public_key(),
    );
    assert!(repeated.is_ok(), "{repeated:?}");
    assert_eq!(repeated.unwrap(), first);
}

fn names_starting_with(directory: &Path, prefix: &str) -> Vec<String> {
    fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with(prefix))
        .collect()
}

#[test]
fn toolchain_uninstall_removes_complete_and_damaged_entries() {
    let root = TempDir::new().unwrap();
    let root = root.path();
    installed(root, "0.23.0");
    installed(root, "0.23.1");
    let versions = root.join("versions");
    let damaged = versions.join("0.23.2");
    fs::create_dir(&damaged).unwrap();
    fs::write(damaged.join(binary_name()), b"damaged").unwrap();
    fs::write(versions.join("0.23.3"), b"regular file").unwrap();
    for stage in [".tmp-0.23.0-12345", ".tmp-script-0.23.0-1"] {
        fs::create_dir(versions.join(stage)).unwrap();
        fs::write(versions.join(stage).join("sentinel"), b"x").unwrap();
    }
    let canonical_versions = versions.canonicalize().unwrap();
    let before_0_23_1 = directory_bytes(&canonical_versions.join("0.23.1"));
    let removed = uninstall(root, release("0.23.0"), &running()).unwrap();
    assert_eq!(removed, Some(canonical_versions.join("0.23.0")));
    assert!(!canonical_versions.join("0.23.0").exists());
    assert_eq!(
        uninstall(root, release("0.23.2"), &running()).unwrap(),
        Some(canonical_versions.join("0.23.2"))
    );
    assert_eq!(
        uninstall(root, release("0.23.3"), &running()).unwrap(),
        Some(canonical_versions.join("0.23.3"))
    );
    assert_eq!(
        uninstall(root, release("0.23.4"), &running()).unwrap(),
        None
    );
    assert_eq!(
        directory_bytes(&canonical_versions.join("0.23.1")),
        before_0_23_1
    );
    assert!(!canonical_versions.join(".tmp-0.23.0-12345").exists());
    assert!(
        canonical_versions
            .join(".tmp-script-0.23.0-1/sentinel")
            .exists()
    );
    assert!(names_starting_with(&canonical_versions, ".tmp-0.23.0-").is_empty());
    let empty_root = TempDir::new().unwrap();
    let empty_root = empty_root.path();
    assert_eq!(
        uninstall(empty_root, release("0.23.0"), &running()).unwrap(),
        None
    );
    assert!(!empty_root.join("versions").exists());
}

#[cfg(unix)]
#[test]
fn toolchain_uninstall_removes_a_symlink_entry_without_following_it() {
    let root = TempDir::new().unwrap();
    let outside = TempDir::new().unwrap();
    let outside = outside.path();
    fs::write(outside.join("sentinel"), b"keep").unwrap();
    let versions = root.path().join("versions");
    fs::create_dir(&versions).unwrap();
    let canonical_versions = versions.canonicalize().unwrap();
    std::os::unix::fs::symlink(outside, versions.join("0.23.0")).unwrap();
    let removed = uninstall(root.path(), release("0.23.0"), &running()).unwrap();
    assert_eq!(removed, Some(canonical_versions.join("0.23.0")));
    assert!(std::fs::symlink_metadata(canonical_versions.join("0.23.0")).is_err());
    assert_eq!(fs::read(outside.join("sentinel")).unwrap(), b"keep");
}

#[test]
fn toolchain_uninstall_refuses_the_running_release() {
    let root = TempDir::new().unwrap();
    let dir = root.path().join("versions/0.23.0");
    fs::create_dir_all(&dir).unwrap();
    let copy = dir.join(binary_name());
    fs::copy(env!("CARGO_BIN_EXE_ridl"), &copy).unwrap();
    fs::write(dir.join(".complete"), b"").unwrap();
    let before = directory_bytes(&dir);
    let cli = std::process::Command::new(&copy)
        .current_dir(root.path())
        .env("RIDL_TOOLCHAIN_DIR", root.path())
        .args(["toolchain", "uninstall", "0.23.0"])
        .output()
        .unwrap();
    assert_eq!(cli.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&cli.stderr).contains("running"));
    assert_eq!(directory_bytes(&dir), before);
    assert!(uninstall(root.path(), release("0.23.0"), &copy).is_err());
    assert_eq!(directory_bytes(&dir), before);
}

#[test]
fn toolchain_uninstall_waits_for_the_version_lock() {
    let root = TempDir::new().unwrap();
    installed(root.path(), "0.23.0");
    let versions = root.path().join("versions");
    let canonical_versions = versions.canonicalize().unwrap();
    let dir = canonical_versions.join("0.23.0");
    let lock = fs::File::create(versions.join(".0.23.0.lock")).unwrap();
    lock.lock().unwrap();
    let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_ridl"))
        .current_dir(root.path())
        .env("RIDL_TOOLCHAIN_DIR", root.path())
        .args(["toolchain", "uninstall", "0.23.0"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    // Process start-up alone can take several hundred milliseconds, so the wait
    // must exceed it for the child to have reached the lock.
    std::thread::sleep(std::time::Duration::from_millis(2000));
    assert!(
        child.try_wait().unwrap().is_none(),
        "uninstall did not wait for the lock"
    );
    assert!(dir.exists());
    lock.unlock().unwrap();
    let output = child.wait_with_output().unwrap();
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        format!("{}\n", dir.display())
    );
    assert!(!dir.exists());
}

#[test]
fn toolchain_uninstall_cli_streams_and_exit_codes() {
    let root = TempDir::new().unwrap();
    let w = TempDir::new().unwrap();
    installed(root.path(), "0.23.0");
    let absent = cli(w.path(), root.path(), &["toolchain", "uninstall", "0.23.4"]);
    output(&absent, 1, "", true);
    assert!(String::from_utf8_lossy(&absent.stderr).contains("ridl 0.23.4 is not installed"));
    for invalid in ["0.23", "0.22.0", "v0.23.0"] {
        let result = cli(w.path(), root.path(), &["toolchain", "uninstall", invalid]);
        output(&result, 2, "", true);
    }
    assert!(root.path().join("versions/0.23.0").exists());
}
