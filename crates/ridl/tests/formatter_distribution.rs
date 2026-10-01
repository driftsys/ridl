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
    let (_, licence) = notices.split_once(
        "The following licence text is copied from that release's LICENSE.txt.\n-------------------------------------------------------------------------------\n\n"
    ).expect("the ec4rs licence follows its attribution");
    // SHA-256 of LICENSE.txt at ec4rs source commit 14bbca047324cedd5791b89f858eb5e17e6b0b3c.
    assert_eq!(
        format!("{:x}", Sha256::digest(licence.as_bytes())),
        "8c6db340475136df3c1201d458fa5755698eace76e510471ecc9d857d6083dac"
    );
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
