//! Calibration's executable boundary over synthetic offline workspaces.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

const CHECKS: [&str; 5] = [
    "inconsistent-unit",
    "inconsistent-abbreviation",
    "duplicate-shape",
    "low-cohesion-interface",
    "package-fan-out",
];

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join(".superpowers/sdd/2026-10-04-design-lints-plan")
            .join(format!(
                "task-12-fix-cli-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn write(&self, path: &str, text: &str) {
        let path = self.0.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    fn command(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_xtask"))
            .current_dir(&self.0)
            .env("CARGO", env!("CARGO"))
            .env("CARGO_NET_OFFLINE", "true")
            .env("CARGO_TARGET_DIR", self.0.join("unselected-target"))
            .args(args)
            .output()
            .unwrap()
    }
    fn derive() -> Self {
        let fixture = Self::new();
        fixture.write("evals/corpus/sample/p/a.typl", "package p;\n");
        fixture.write(
            "evals/tasks/review-0001/task.toml",
            "id = \"review-0001\"\nkind = \"review\"\ncorpus = \"sample\"\n",
        );
        fixture.write(
            "evals/tasks/review-0001/rubric.md",
            "1. **must** identify the issue.\n",
        );
        let mut recall = String::from(
            "[[item]]\nid = \"review-0001:1\"\nworkspace = \"sample\"\nkind = \"issue\"\nreason = \"Reviewed design issue\"\n",
        );
        for check in CHECKS {
            let (labels, matches) = if check == "package-fan-out" {
                (
                    "[[finding]]\nid = \"package-fan-out:sample:p/a.typl:0-7:0\"\nworkspace = \"sample\"\nlocation = \"p/a.typl:1\"\nmessage = \"package `p` depends on 4 workspace packages: a, b, c, d\"\nclaude = \"accept\"\nclaude_reason = \"reason\"\nsol = \"accept\"\nsol_reason = \"reason\"\nfinal = \"accept\"\n[finding.metric]\nkind = \"fan-out\"\ncount = 4\n",
                    "[\"package-fan-out:sample:p/a.typl:0-7:0\"]",
                )
            } else {
                ("finding = []\n", "[]")
            };
            fixture.write(&format!("evals/calibration/{check}.toml"), labels);
            recall.push_str(&format!("\n[[check]]\nname = \"{check}\"\n[[check.issue]]\nid = \"review-0001:1\"\napplicable = {}\nreason = \"Reviewed applicability\"\nfindings = {matches}\n",check=="package-fan-out"));
        }
        fixture.write("evals/calibration/recall.toml", &recall);
        fixture
    }
    fn dump() -> Self {
        let fixture = Self::new();
        fixture.write(
            "Cargo.toml",
            "[workspace]\nmembers = [\"ridl-cli\"]\nresolver = \"3\"\n",
        );
        fixture.write(
            "Cargo.lock",
            "version = 4\n\n[[package]]\nname = \"ridl-cli\"\nversion = \"0.0.0\"\n",
        );
        fixture.write(".cargo/config.toml", "[net]\noffline = true\n");
        fixture.write("ridl-cli/Cargo.toml","[package]\nname = \"ridl-cli\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n[[bin]]\nname = \"ridl\"\npath = \"src/main.rs\"\n");
        // This std-only child is a protocol fixture for the external compiler.
        // The real xtask executable still builds it, copies workspaces, edits
        // copied manifests, validates diagnostics and publishes the arrays.
        fixture.write("ridl-cli/src/main.rs",r#"
use std::{env,fs,path::PathBuf};
fn main() {
    let args:Vec<_>=env::args().skip(1).collect();
    assert_eq!(&args[..3], ["check","--format","json"]);
    let root=PathBuf::from(&args[3]);
    assert_eq!(root,env::current_dir().unwrap());
    assert!(root.parent().unwrap().file_name().unwrap().to_str().unwrap().starts_with(".calibrate-work-"));
    let manifest=fs::read_to_string(root.join("ridl.toml")).unwrap();
    for check in ["inconsistent-unit","inconsistent-abbreviation","duplicate-shape","low-cohesion-interface","package-fan-out"] {
        assert!(manifest.contains(&format!("{check} = \"warn\"")));
    }
    fs::write(root.join("copy-only.marker"),"checked copy").unwrap();
    let mode=fs::read_to_string(root.join("mode.txt")).unwrap();
    if mode=="failed" { eprintln!("synthetic check failure");std::process::exit(1); }
    let (severity,lint,message)=match mode.as_str() {
        "malformed" => ("warning","low-cohesion-interface","interface `I` splits into 2 groups of members that share no type: a], [b]"),
        "error" => ("error","package-fan-out","package `p` depends on 4 workspace packages: a, b, c, d"),
        _ => ("warning","package-fan-out","package `p` depends on 4 workspace packages: a, b, c, d"),
    };
    let path=root.join("p/a.typl").to_string_lossy().into_owned();
    println!("[{{\"severity\":{severity:?},\"lint\":{lint:?},\"message\":{message:?},\"span\":{{\"path\":{path:?},\"start\":{{\"line\":1,\"column\":1}},\"end\":{{\"line\":1,\"column\":8}}}}}}]");
}
"#);
        for workspace in ["alpha", "beta"] {
            fixture.write(
                &format!("evals/corpus/{workspace}/ridl.toml"),
                "[workspace]\nmembers = [\"p\"]\n",
            );
            fixture.write(
                &format!("evals/corpus/{workspace}/p/a.typl"),
                "package p;\n",
            );
            fixture.write(&format!("evals/corpus/{workspace}/mode.txt"), "valid");
        }
        fixture
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn success(output: &Output) {
    assert!(
        output.status.success(),
        "status {:?}: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
}
fn snapshot(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn walk(root: &Path, path: &Path, entries: &mut BTreeMap<PathBuf, Vec<u8>>) {
        let metadata = fs::symlink_metadata(path).unwrap();
        let value = if metadata.is_symlink() {
            fs::read_link(path)
                .unwrap()
                .to_string_lossy()
                .as_bytes()
                .to_vec()
        } else if metadata.is_file() {
            fs::read(path).unwrap()
        } else {
            Vec::new()
        };
        entries.insert(path.strip_prefix(root).unwrap().into(), value);
        if metadata.is_dir() {
            for entry in fs::read_dir(path).unwrap() {
                walk(root, &entry.unwrap().path(), entries);
            }
        }
    }
    let mut entries = BTreeMap::new();
    walk(root, root, &mut entries);
    entries
}
fn no_copies(out: &Path) {
    assert!(!fs::read_dir(out).unwrap().any(|e| {
        e.unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".calibrate-work-")
    }));
}

#[test]
fn executable_derivation_dispatch_alias_and_default_no_write() {
    let fixture = Fixture::derive();
    let summary = fixture.0.join("evals/calibration/summary.md");
    let canonical = fixture.command(&["calibrate", "derive"]);
    success(&canonical);
    assert!(
        String::from_utf8_lossy(&canonical.stdout)
            .contains("Selected level: Info. Threshold: fan-out > 3")
    );
    assert!(!summary.exists());
    let alias = fixture.command(&["calibrate", "--derive"]);
    success(&alias);
    assert_eq!(alias.stdout, canonical.stdout);
    assert!(!summary.exists());
    for action in ["derive", "--derive"] {
        let output = fixture.command(&["calibrate", action, "--write"]);
        success(&output);
        assert_eq!(output.stdout, canonical.stdout);
        assert_eq!(fs::read(&summary).unwrap(), canonical.stdout);
        fs::remove_file(&summary).unwrap();
    }
    let help = fixture.command(&["calibrate", "--help"]);
    success(&help);
    let help = String::from_utf8_lossy(&help.stdout);
    assert!(help.contains("calibrate derive [--write]"));
    assert!(help.contains("calibrate --derive [--write]"));
    let invalid = fixture.command(&["calibrate", "derive", "--unknown"]);
    assert_eq!(invalid.status.code(), Some(2));
    assert!(!summary.exists());
}

#[test]
fn executable_dump_rejects_corpus_destinations_without_writes() {
    let fixture = Fixture::dump();
    let before = snapshot(&fixture.0);
    for destination in [
        "evals/corpus",
        "evals/corpus/alpha/new/deep",
        "evals/corpus/alpha/missing/../new",
    ] {
        let output = fixture.command(&["calibrate", "dump", destination]);
        assert_eq!(output.status.code(), Some(2));
        assert!(String::from_utf8_lossy(&output.stderr).contains("inside the corpus"));
        assert_eq!(snapshot(&fixture.0), before);
    }
}

#[cfg(unix)]
#[test]
fn executable_dump_rejects_symlink_destinations_without_writes() {
    let fixture = Fixture::dump();
    std::os::unix::fs::symlink(
        fixture.0.join("evals/corpus/alpha"),
        fixture.0.join("alias"),
    )
    .unwrap();
    let before = snapshot(&fixture.0);
    for destination in ["alias/new", "missing/../alias/new"] {
        let output = fixture.command(&["calibrate", "dump", destination]);
        assert_eq!(output.status.code(), Some(2));
        assert!(String::from_utf8_lossy(&output.stderr).contains("inside the corpus"));
        assert_eq!(snapshot(&fixture.0), before);
    }
}

#[test]
fn executable_dump_isolates_copies_target_and_cleans_up_after_success() {
    let fixture = Fixture::dump();
    let corpus = fixture.0.join("evals/corpus");
    let before = snapshot(&corpus);
    let output = fixture.command(&["calibrate", "dump", "out"]);
    success(&output);
    let out = fixture.0.join("out");
    no_copies(&out);
    assert_eq!(snapshot(&corpus), before);
    assert!(!fixture.0.join("unselected-target").exists());
    assert!(
        out.join(".calibrate-target/debug")
            .join(format!("ridl{}", std::env::consts::EXE_SUFFIX))
            .is_file()
    );
    for check in CHECKS {
        let records: Vec<serde_json::Value> =
            serde_json::from_slice(&fs::read(out.join(format!("{check}.json"))).unwrap()).unwrap();
        if check == "package-fan-out" {
            assert_eq!(records.len(), 2);
            for (record, workspace) in records.iter().zip(["alpha", "beta"]) {
                assert_eq!(
                    record["id"],
                    format!("package-fan-out:{workspace}:p/a.typl:0-7:0")
                );
                assert_eq!(
                    record["metric"],
                    serde_json::json!({"kind":"fan-out","count":4})
                );
                assert_eq!(record["location"], "p/a.typl:1");
            }
        } else {
            assert!(records.is_empty());
        }
    }
}

#[test]
fn executable_dump_refuses_existing_lints_and_delays_publication_on_failures() {
    let fixture = Fixture::dump();
    let beta = fixture.0.join("evals/corpus/beta");
    let manifest = fs::read_to_string(beta.join("ridl.toml")).unwrap();
    let out = fixture.0.join("out");
    fs::create_dir(&out).unwrap();
    for check in CHECKS {
        fs::write(
            out.join(format!("{check}.json")),
            "unchanged previous array",
        )
        .unwrap();
    }
    for (mode, reason) in [
        ("lints", "already has a lints table"),
        ("malformed", "missing or malformed metric"),
        ("error", "corpus has an error"),
        ("failed", "synthetic check failure"),
    ] {
        fs::write(
            beta.join("mode.txt"),
            if mode == "lints" { "valid" } else { mode },
        )
        .unwrap();
        fs::write(
            beta.join("ridl.toml"),
            if mode == "lints" {
                format!("{manifest}\n[lints]\npackage-fan-out = \"allow\"\n")
            } else {
                manifest.clone()
            },
        )
        .unwrap();
        let before = snapshot(&fixture.0.join("evals/corpus"));
        let output = fixture.command(&["calibrate", "dump", "out"]);
        assert_eq!(output.status.code(), Some(2));
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(reason),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(snapshot(&fixture.0.join("evals/corpus")), before);
        no_copies(&out);
        for check in CHECKS {
            assert_eq!(
                fs::read_to_string(out.join(format!("{check}.json"))).unwrap(),
                "unchanged previous array"
            );
        }
    }
}

#[cfg(unix)]
#[test]
fn executable_dump_refuses_corpus_symlinks_and_removes_copies() {
    let fixture = Fixture::dump();
    fixture.write("external.typl", "external source");
    std::os::unix::fs::symlink(
        fixture.0.join("external.typl"),
        fixture.0.join("evals/corpus/beta/p/link.typl"),
    )
    .unwrap();
    let before = snapshot(&fixture.0.join("evals/corpus"));
    let output = fixture.command(&["calibrate", "dump", "out"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("corpus contains a symlink"));
    assert_eq!(snapshot(&fixture.0.join("evals/corpus")), before);
    let out = fixture.0.join("out");
    no_copies(&out);
    assert!(
        CHECKS
            .iter()
            .all(|c| !out.join(format!("{c}.json")).exists())
    );
}

#[cfg(unix)]
#[test]
fn executable_dump_rejects_linked_publication_and_build_destinations_before_side_effects() {
    for destination in [
        "inconsistent-unit.json",
        "package-fan-out.json",
        ".calibrate-target",
        ".calibrate-target/debug",
        ".calibrate-target/debug/ridl",
        ".calibrate-target/debug/build/linked-output",
    ] {
        let fixture = Fixture::dump();
        let out = fixture.0.join("out");
        fs::create_dir(&out).unwrap();
        let link = out.join(destination);
        fs::create_dir_all(link.parent().unwrap()).unwrap();
        let source = if destination.ends_with(".json") || destination.ends_with("/ridl") {
            fixture.0.join("evals/corpus/alpha/p/a.typl")
        } else {
            fixture.0.join("evals/corpus/alpha")
        };
        std::os::unix::fs::symlink(&source, &link).unwrap();
        let before = snapshot(&fixture.0);
        let output = fixture.command(&["calibrate", "dump", "out"]);
        let after = snapshot(&fixture.0);
        let changed = before
            .keys()
            .chain(after.keys())
            .filter(|p| before.get(*p) != after.get(*p))
            .collect::<std::collections::BTreeSet<_>>();
        assert!(
            changed.is_empty(),
            "unsafe side effects for {destination}: {changed:?}"
        );
        assert_eq!(output.status.code(), Some(2), "{destination}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("symlink"),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[cfg(unix)]
#[test]
fn executable_dump_rejects_hard_linked_files_before_build_or_publication() {
    for destination in ["package-fan-out.json", ".calibrate-target/debug/ridl"] {
        let fixture = Fixture::dump();
        let link = fixture.0.join("out").join(destination);
        fs::create_dir_all(link.parent().unwrap()).unwrap();
        fs::hard_link(fixture.0.join("evals/corpus/alpha/p/a.typl"), &link).unwrap();
        let before = snapshot(&fixture.0);
        let output = fixture.command(&["calibrate", "dump", "out"]);
        assert!(
            snapshot(&fixture.0) == before,
            "side effect for {destination}"
        );
        assert_eq!(output.status.code(), Some(2));
        assert!(String::from_utf8_lossy(&output.stderr).contains("hard link"));
    }
}
