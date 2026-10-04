//! Default formatter API availability and distributed licence contents.

use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

struct TestDir(PathBuf);

impl TestDir {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "ridl-formatter-distribution-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn write(&self, path: &str, bytes: &[u8]) {
        let path = self.0.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn the_cli_dependency_exposes_the_default_editorconfig_api() {
    let dir = TestDir::new();
    dir.write(".editorconfig", b"root = true\n[*]\nmax_line_length = 60\n");
    assert_eq!(
        ridl_fmt::FormatOptions::for_path(&dir.0.join("file.ridl")).max_line_length,
        Some(60)
    );
}

#[test]
fn ec4rs_licence_text_matches_the_upstream_release() {
    let notices = std::fs::read_to_string(repo().join("THIRD-PARTY-NOTICES.txt")).unwrap();
    assert!(
        notices.contains(concat!(
            "ec4rs 1.2.0 — EditorConfig For Rust\n",
            "Author: TheDaemoness\n",
            "Source: https://github.com/TheDaemoness/ec4rs\n",
            "Release source commit: 14bbca047324cedd5791b89f858eb5e17e6b0b3c\n",
            "Licence: Apache-2.0\n",
        )),
        "the ec4rs attribution must accompany its licence"
    );
    let licence = ec4rs_licence_text(&notices);
    // SHA-256 of LICENSE.txt at ec4rs source commit 14bbca047324cedd5791b89f858eb5e17e6b0b3c.
    assert_eq!(
        format!("{:x}", Sha256::digest(licence.as_bytes())),
        "8c6db340475136df3c1201d458fa5755698eace76e510471ecc9d857d6083dac"
    );
    // Retain the authentic licence, including its final newline, as the last notice.
    let licence_end = notices.find(licence).unwrap() + licence.len();
    assert_eq!(
        format!(
            "{:x}",
            Sha256::digest(ec4rs_licence_text(&notices[..licence_end]).as_bytes())
        ),
        "8c6db340475136df3c1201d458fa5755698eace76e510471ecc9d857d6083dac",
        "the complete upstream licence must also match at end of file"
    );
}

fn ec4rs_licence_text(notices: &str) -> &str {
    let (_, licence) = notices.split_once(
        "The following licence text is copied from that release's LICENSE.txt.\n-------------------------------------------------------------------------------\n\n"
    ).expect("the ec4rs licence follows its attribution");
    // A later notice starts at a separator; preserve the licence's final newline.
    licence
        .split_once(
            "\n-------------------------------------------------------------------------------\n",
        )
        .map_or(licence, |(licence, _)| licence)
}

#[test]
fn ec4rs_licence_check_rejects_modified_or_truncated_text() {
    let notices = std::fs::read_to_string(repo().join("THIRD-PARTY-NOTICES.txt")).unwrap();
    for (original, replacement) in [
        ("Version 2.0, January 2004", "Version 3.0, January 2004"),
        ("   limitations under the License.\n", ""),
    ] {
        assert!(notices.contains(original));
        let changed = notices.replacen(original, replacement, 1);
        assert_ne!(
            format!(
                "{:x}",
                Sha256::digest(ec4rs_licence_text(&changed).as_bytes())
            ),
            "8c6db340475136df3c1201d458fa5755698eace76e510471ecc9d857d6083dac",
            "the upstream licence must remain byte-exact"
        );
    }
}

#[cfg(unix)]
#[test]
fn both_release_archives_carry_the_licences() {
    use std::process::Command;

    let workflow =
        std::fs::read_to_string(repo().join(".github/workflows/vscode-release.yaml")).unwrap();
    let (_, packaging) = workflow
        .split_once("      - name: tarball and checksum\n")
        .unwrap();
    let (_, commands) = packaging.split_once("        run: |\n").unwrap();
    let script = commands
        .lines()
        .take_while(|line| line.is_empty() || line.starts_with("          "))
        .map(|line| line.strip_prefix("          ").unwrap_or(line))
        .collect::<Vec<_>>()
        .join("\n");

    for extension in ["", ".exe"] {
        let dir = TestDir::new();
        for binary in ["ridl", "ridlc"] {
            dir.write(
                &format!("target/fixture/release/{binary}{extension}"),
                b"fixture binary\n",
            );
        }
        for document in ["LICENSE", "THIRD-PARTY-NOTICES.txt"] {
            dir.write(document, &std::fs::read(repo().join(document)).unwrap());
        }
        let script = script
            .replace("${{ matrix.target }}", "fixture")
            .replace("${{ matrix.binary }}", &format!("ridl{extension}"))
            .replace("${{ matrix.ridlc-binary }}", &format!("ridlc{extension}"));
        let output = Command::new("bash")
            .args(["-eu", "-c", &script])
            .current_dir(&dir.0)
            .env("COPYFILE_DISABLE", "1")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        for binary in ["ridl", "ridlc"] {
            let archive = dir.0.join(format!("{binary}-fixture.tar.gz"));
            for member in [
                format!("{binary}{extension}"),
                "LICENSE".into(),
                "THIRD-PARTY-NOTICES.txt".into(),
            ] {
                let output = Command::new("tar")
                    .arg("-xOf")
                    .arg(&archive)
                    .arg(&member)
                    .output()
                    .unwrap();
                assert!(output.status.success(), "{binary}: missing {member}");
                let original = if member.starts_with(binary) {
                    dir.0.join("target/fixture/release").join(&member)
                } else {
                    repo().join(&member)
                };
                assert_eq!(
                    output.stdout,
                    std::fs::read(original).unwrap(),
                    "{binary}: {member}"
                );
            }
        }
    }
}

#[test]
fn the_root_licence_is_the_recorded_mit_licence() {
    let licence = std::fs::read(repo().join("LICENSE")).unwrap();
    // The complete MIT text and driftsys copyright recorded on 2026-10-01.
    assert_eq!(
        format!("{:x}", Sha256::digest(&licence)),
        "c502d160325cfb7af6776061fa114aa0a36dbf25811c86e950e1a6e70cd34f87"
    );
}

#[cfg(unix)]
#[test]
fn vsix_packaging_passes_the_target_and_output_arguments() {
    use std::os::unix::fs::PermissionsExt;
    use std::process::Command;

    let dir = TestDir::new();
    dir.write("justfile", &std::fs::read(repo().join("justfile")).unwrap());
    dir.write("THIRD-PARTY-NOTICES.txt", b"fixture notices\n");
    // Exercise the real recipe without installing npm dependencies or packaging
    // a binary. The acceptance recipe checks the actual VSIX separately.
    // Each fake tool fails outside editors/vscode and logs what it was asked to
    // run, so the recipe's directory change and its npm steps are pinned too.
    let guard = "case \"$PWD\" in */editors/vscode) ;; *) echo \"wrong directory: $PWD\" >&2; exit 1 ;; esac\n";
    for (name, script) in [
        (
            "npm",
            format!("#!/bin/sh\n{guard}echo \"npm $*\" >> \"$ARGUMENT_LOG\"\n"),
        ),
        (
            "npx",
            format!("#!/bin/sh\n{guard}printf '%s\\n' \"$@\" >> \"$ARGUMENT_LOG\"\n"),
        ),
    ] {
        dir.write(&format!("tools/{name}"), script.as_bytes());
        std::fs::set_permissions(
            dir.0.join(format!("tools/{name}")),
            std::fs::Permissions::from_mode(0o755),
        )
        .unwrap();
    }
    let search_path = std::env::join_paths(std::iter::once(dir.0.join("tools")).chain(
        std::env::split_paths(&std::env::var_os("PATH").expect("PATH is set")),
    ))
    .unwrap();
    let log = dir.0.join("arguments.txt");
    for (target, output, expected) in [
        ("", "", vec!["vsce", "package"]),
        (
            "linux-x64",
            "",
            vec![
                "vsce",
                "package",
                "--target",
                "linux-x64",
                "--out",
                "ridl-lang-linux-x64.vsix",
            ],
        ),
        (
            "",
            "custom.vsix",
            vec!["vsce", "package", "--out", "custom.vsix"],
        ),
        (
            "darwin-arm64",
            "custom target.vsix",
            vec![
                "vsce",
                "package",
                "--target",
                "darwin-arm64",
                "--out",
                "custom target.vsix",
            ],
        ),
    ] {
        let _ = std::fs::remove_file(&log);
        // A copy left by an earlier case must not satisfy this case.
        let _ = std::fs::remove_file(dir.0.join("editors/vscode/bin/THIRD-PARTY-NOTICES.txt"));
        let result = Command::new("just")
            .arg("--justfile")
            .arg(dir.0.join("justfile"))
            .args(["package-vsix", target, output])
            .env("PATH", &search_path)
            .env("ARGUMENT_LOG", &log)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            std::fs::read_to_string(&log)
                .unwrap()
                .lines()
                .collect::<Vec<_>>(),
            ["npm ci", "npm run compile"]
                .into_iter()
                .chain(expected)
                .collect::<Vec<_>>(),
            "target {target:?}, output {output:?}"
        );
        assert_eq!(
            std::fs::read(dir.0.join("editors/vscode/bin/THIRD-PARTY-NOTICES.txt")).unwrap(),
            b"fixture notices\n"
        );
    }
}
