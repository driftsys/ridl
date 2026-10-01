//! Width resolution follows EditorConfig matching and precedence.

#![cfg(feature = "editorconfig")]

use ridl_fmt::FormatOptions;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

struct TestDir(PathBuf);

impl TestDir {
    fn new() -> Self {
        Self::new_in(&std::env::temp_dir())
    }

    fn new_in(parent: &Path) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = parent.join(format!(
            "ridl-fmt-editorconfig-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn write(&self, relative: &str, text: &str) -> PathBuf {
        let path = self.0.join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, text).unwrap();
        path
    }

    fn width(&self, relative: &str) -> Option<usize> {
        FormatOptions::for_path(&self.0.join(relative)).max_line_length
    }
}

#[test]
fn relative_and_absolute_paths_match_the_same_directory_glob() {
    let dir = TestDir::new_in(Path::new("."));
    dir.write(
        "nested/.editorconfig",
        "root = true\n[models/*.ridl]\nmax_line_length = 60\n",
    );
    let relative = dir.0.join("nested/models/file.ridl");
    assert!(relative.is_relative());
    let absolute = std::env::current_dir().unwrap().join(&relative);
    assert_eq!(FormatOptions::for_path(&absolute).max_line_length, Some(60));
    assert_eq!(FormatOptions::for_path(&relative).max_line_length, Some(60));
}

#[test]
fn a_configuration_parse_error_uses_the_default() {
    let dir = TestDir::new();
    let config = dir.write(
        ".editorconfig",
        "root = true\n[*]\nmax_line_length = 60\ninvalid line\n",
    );
    let file = config.parent().unwrap().join("file.ridl");
    assert!(
        ec4rs::properties_of(&file).is_err(),
        "fixture must reach the error path"
    );
    assert_eq!(FormatOptions::for_path(&file).max_line_length, Some(100));
}

impl Drop for TestDir {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn the_extension_selects_the_matching_section() {
    let dir = TestDir::new();
    dir.write(
        ".editorconfig",
        "root = true\n[*.ridl]\nmax_line_length = 60\n",
    );
    assert_eq!(dir.width("file.ridl"), Some(60));
    assert_eq!(dir.width("file.typl"), Some(100));
}

#[test]
fn a_brace_glob_matches_rsdl() {
    let dir = TestDir::new();
    dir.write(
        ".editorconfig",
        "root = true\n[*.{typl,ridl,rsdl}]\nmax_line_length = 40\n",
    );
    assert_eq!(dir.width("file.rsdl"), Some(40));
}

#[test]
fn off_disables_the_width_limit() {
    let dir = TestDir::new();
    dir.write(".editorconfig", "root = true\n[*]\nmax_line_length = off\n");
    assert_eq!(dir.width("file.typl"), None);
}

#[test]
fn unset_and_invalid_values_restore_the_default() {
    for value in ["unset", "abc"] {
        let dir = TestDir::new();
        dir.write(".editorconfig", "root = true\n[*]\nmax_line_length = 60\n");
        dir.write(
            "nested/.editorconfig",
            &format!("[*]\nmax_line_length = {value}\n"),
        );
        assert_eq!(dir.width("nested/file.typl"), Some(100), "{value}");
    }
}

#[test]
fn a_nested_root_stops_the_parent_search() {
    let dir = TestDir::new();
    dir.write(".editorconfig", "root = true\n[*]\nmax_line_length = 60\n");
    dir.write(
        "nested/.editorconfig",
        "root = true\n[*]\nindent_size = 4\n",
    );
    assert_eq!(dir.width("nested/file.typl"), Some(100));
}

#[test]
fn a_later_section_overrides_an_earlier_section() {
    let dir = TestDir::new();
    dir.write(
        ".editorconfig",
        "root = true\n[*]\nmax_line_length = 40\n[*.typl]\nmax_line_length = 60\n",
    );
    assert_eq!(dir.width("file.typl"), Some(60));
}

#[test]
fn no_editorconfig_uses_the_default() {
    let dir = TestDir::new();
    assert_eq!(dir.width("file.typl"), Some(100));
}
