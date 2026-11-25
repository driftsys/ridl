//! The release workflow signs each `ridl` archive with minisign and refuses to
//! sign while the public key in the CLI is a placeholder.

#![cfg(unix)]

use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};
use tempfile::TempDir;

const TARGETS: [&str; 5] = [
    "aarch64-apple-darwin",
    "x86_64-apple-darwin",
    "aarch64-unknown-linux-gnu",
    "x86_64-unknown-linux-gnu",
    "x86_64-pc-windows-msvc",
];
const PLACEHOLDER: &str = "RIDL-RELEASE-KEY-PLACEHOLDER";

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn workflow() -> String {
    fs::read_to_string(repo().join(".github/workflows/vscode-release.yaml")).unwrap()
}

/// The `run: |` script of the step `name`, without its indentation.
fn script(name: &str) -> String {
    let workflow = workflow();
    let (_, step) = workflow
        .split_once(&format!("      - name: {name}\n"))
        .unwrap_or_else(|| panic!("no step named {name}"));
    let (_, commands) = step.split_once("        run: |\n").unwrap();
    commands
        .lines()
        .take_while(|line| line.is_empty() || line.starts_with("          "))
        .map(|line| line.strip_prefix("          ").unwrap_or(line))
        .collect::<Vec<_>>()
        .join("\n")
}

struct Run {
    work: TempDir,
    record: PathBuf,
    output: Output,
}

struct Setup<'a> {
    key: &'a str,
    password: &'a str,
    archives: bool,
    mod_rs: &'a str,
}

const DEFAULT: Setup = Setup {
    key: "fake-key",
    password: "fake-password",
    archives: true,
    mod_rs: "pub const RELEASE_PUBLIC_KEY: &str = \"RWQexamplekey\";\n",
};

// `minisign` is replaced by a program that records its arguments, one per
// line followed by `--`, its standard input, and the content of the key file
// it was given, and then writes the signature file named by `-x`. The record
// directory is outside the working directory so that a scan of the working
// directory finds only what the script itself left behind.
const FAKE_MINISIGN: &str = r#"#!/bin/sh
out=""
prev=""
for argument in "$@"; do
  printf '%s\n' "$argument" >> "$RECORD"
  [ "$prev" = -x ] && out="$argument"
  [ "$prev" = -s ] && cat "$argument" >> "$RECORD.key"
  prev="$argument"
done
printf -- '--\n' >> "$RECORD"
{ cat; printf -- '--\n'; } >> "$RECORD.stdin"
printf 'signature' > "$out"
"#;

fn run(setup: &Setup) -> Run {
    use std::os::unix::fs::PermissionsExt;
    let work = TempDir::new().unwrap();
    let support = TempDir::new().unwrap();
    let bin = support.path().join("bin");
    fs::create_dir(&bin).unwrap();
    fs::write(bin.join("minisign"), FAKE_MINISIGN).unwrap();
    fs::set_permissions(bin.join("minisign"), fs::Permissions::from_mode(0o755)).unwrap();
    fs::create_dir(work.path().join("artifacts")).unwrap();
    fs::create_dir(work.path().join("tmp")).unwrap();
    if setup.archives {
        for target in TARGETS {
            for binary in ["ridl", "ridlc"] {
                fs::write(
                    work.path()
                        .join(format!("artifacts/{binary}-{target}.tar.gz")),
                    b"archive",
                )
                .unwrap();
            }
        }
    }
    let toolchain = work.path().join("crates/ridl/src/toolchain");
    fs::create_dir_all(&toolchain).unwrap();
    fs::write(toolchain.join("mod.rs"), setup.mod_rs).unwrap();
    let record = support.path().join("record");
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").unwrap());
    let output = Command::new("bash")
        .args(["-eu", "-c", &script("sign the ridl archives")])
        .current_dir(work.path())
        .env("PATH", path)
        .env("TMPDIR", work.path().join("tmp"))
        .env("RECORD", &record)
        .env("RIDL_RELEASE_SIGNING_KEY", setup.key)
        .env("RIDL_RELEASE_SIGNING_KEY_PASSWORD", setup.password)
        .env("RELEASE_TAG", "editor-v0.23.0")
        .output()
        .unwrap();
    // The record files move to a directory beside the working directory, so a
    // scan of the working directory sees only what the script left behind.
    let kept = work.path().join("..").join(format!(
        "{}-record",
        work.path().file_name().unwrap().to_string_lossy()
    ));
    fs::create_dir_all(&kept).unwrap();
    for suffix in ["", ".key", ".stdin"] {
        let from = PathBuf::from(format!("{}{suffix}", record.display()));
        if from.exists() {
            fs::copy(&from, kept.join(format!("record{suffix}"))).unwrap();
        }
    }
    Run {
        work,
        record: kept.join("record"),
        output,
    }
}

impl Run {
    fn stderr(&self) -> String {
        String::from_utf8_lossy(&self.output.stderr).into_owned()
    }
    fn minisig_files(&self) -> Vec<String> {
        fs::read_dir(self.work.path().join("artifacts"))
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|n| n.ends_with(".minisig"))
            .collect()
    }
    fn invocations(&self) -> Vec<Vec<String>> {
        let text = fs::read_to_string(&self.record).unwrap();
        text.split("--\n")
            .filter(|chunk| !chunk.is_empty())
            .map(|chunk| chunk.lines().map(str::to_string).collect())
            .collect()
    }
}

impl Drop for Run {
    fn drop(&mut self) {
        if let Some(parent) = self.record.parent() {
            let _ = fs::remove_dir_all(parent);
        }
    }
}

fn any_file_contains(dir: &Path, needle: &str) -> bool {
    fs::read_dir(dir).unwrap().any(|entry| {
        let path = entry.unwrap().path();
        if path.is_dir() {
            any_file_contains(&path, needle)
        } else {
            fs::read(&path)
                .map(|bytes| String::from_utf8_lossy(&bytes).contains(needle))
                .unwrap_or(false)
        }
    })
}

#[test]
fn signing_step_fails_without_the_secret() {
    let run = run(&Setup { key: "", ..DEFAULT });
    assert!(!run.output.status.success());
    assert!(
        run.stderr().contains("RIDL_RELEASE_SIGNING_KEY"),
        "{}",
        run.stderr()
    );
    assert!(!run.record.exists());
    assert!(run.minisig_files().is_empty());
}

#[test]
fn signing_step_signs_each_ridl_archive_with_its_trusted_comment() {
    let run = run(&DEFAULT);
    assert!(run.output.status.success(), "{}", run.stderr());
    let invocations = run.invocations();
    assert_eq!(invocations.len(), 5);
    let key_path = invocations[0][2].clone();
    assert_eq!(invocations[0][1], "-s");
    assert!(
        key_path.starts_with(run.work.path().join("tmp").to_str().unwrap()),
        "{key_path}"
    );
    for target in TARGETS {
        let archive = format!("artifacts/ridl-{target}.tar.gz");
        let expected: Vec<String> = [
            "-S",
            "-s",
            &key_path,
            "-m",
            &archive,
            "-t",
            &format!("ridl 0.23.0 {target}"),
            "-x",
            &format!("{archive}.minisig"),
        ]
        .map(String::from)
        .to_vec();
        assert!(invocations.contains(&expected), "{target}: {invocations:?}");
        assert!(
            !run.work
                .path()
                .join(format!("artifacts/ridlc-{target}.tar.gz.minisig"))
                .exists()
        );
    }
    let stdin = fs::read_to_string(run.record.with_file_name("record.stdin")).unwrap();
    let stdins: Vec<_> = stdin.split("--\n").filter(|s| !s.is_empty()).collect();
    assert_eq!(stdins.len(), 5);
    assert!(stdins.iter().all(|s| *s == "fake-password\n"), "{stdins:?}");
    let key_content = fs::read_to_string(run.record.with_file_name("record.key")).unwrap();
    assert!(key_content.contains("fake-key"));
    assert!(!Path::new(&key_path).exists(), "the key file is removed");
    assert!(!any_file_contains(run.work.path(), "fake-key"));
}

#[test]
fn signing_step_fails_when_no_archive_is_present() {
    let run = run(&Setup {
        archives: false,
        ..DEFAULT
    });
    assert!(!run.output.status.success());
}

#[test]
fn signing_step_fails_while_the_public_key_is_a_placeholder() {
    let mod_rs = format!("pub const RELEASE_PUBLIC_KEY: &str = \"{PLACEHOLDER}: replace me\";\n");
    let run = run(&Setup {
        mod_rs: &mod_rs,
        ..DEFAULT
    });
    assert!(!run.output.status.success());
    assert!(run.stderr().contains(PLACEHOLDER), "{}", run.stderr());
    assert!(!run.record.exists());
    assert!(run.minisig_files().is_empty());
}

#[test]
fn release_job_installs_minisign_then_signs_before_upload() {
    let workflow = workflow();
    let (_, job) = workflow.split_once("\n  release:\n").unwrap();
    let (job, _) = job.split_once("\n  publish-vsce:\n").unwrap();
    let position = |needle: &str| {
        job.find(needle)
            .unwrap_or_else(|| panic!("the release job has no {needle}"))
    };
    let order = [
        position("actions/download-artifact"),
        position("name: install minisign"),
        position("name: sign the ridl archives"),
        position("softprops/action-gh-release"),
    ];
    assert!(order.windows(2).all(|pair| pair[0] < pair[1]), "{order:?}");
    assert!(job[order[3]..].contains("artifacts/*.minisig"));
}

#[test]
fn marketplace_publish_waits_for_the_signed_release() {
    let workflow = workflow();
    let (_, job) = workflow.split_once("\n  publish-vsce:\n").unwrap();
    assert!(
        job.contains("needs: [package-vsix, release]"),
        "publish-vsce must need the release job"
    );
}

#[test]
fn minisign_verify_notice_accompanies_the_dependency() {
    let notices = fs::read_to_string(repo().join("THIRD-PARTY-NOTICES.txt")).unwrap();
    assert!(notices.contains(concat!(
        "minisign-verify 0.3.0 \u{2014} Minisign signature verification\n",
        "Source: https://github.com/jedisct1/rust-minisign-verify\n",
        "Licence: MIT\n",
    )));
}
