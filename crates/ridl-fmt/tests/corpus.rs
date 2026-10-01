//! The `ridl fmt` golden corpus: `input/*.{typl,ridl,rsdl}` in the old spaced/aligned
//! style must format to the byte-identical `formatted/*.{typl,ridl,rsdl}` tight style, and
//! every `formatted/*.{typl,ridl,rsdl}` file must be a fixed point (docs/ROADMAP.md epic
//! E1.14, general form §5).

use std::fs;
use std::path::{Path, PathBuf};

use ridl_fmt::{FormatOptions, FormatOutcome, format};
use ridl_syntax::Profile;

fn test_data(sub: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("test_data")
        .join(sub)
}

/// The family source files in a test-data directory, sorted by name.
fn source_files(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("read_dir {}: {e}", dir.display()))
        .map(|entry| entry.expect("dir entry").path())
        .filter(|path| profile_of_path(path).is_some())
        .collect();
    files.sort();
    files
}

fn format_ok(text: &str, profile: Profile, context: &str) -> String {
    match format(text, profile, &FormatOptions::default()) {
        FormatOutcome::Formatted(out) => out,
        FormatOutcome::ParseErrors(errors) => {
            panic!("{context} produced parse errors: {errors:?}")
        }
    }
}

#[test]
fn every_input_formats_to_its_golden() {
    let inputs = source_files(&test_data("input"));
    assert!(!inputs.is_empty(), "the input corpus must not be empty");
    for input_path in inputs {
        let name = input_path
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        let golden_path = test_data("formatted").join(&name);
        let input = fs::read_to_string(&input_path).expect("read input");
        let golden = fs::read_to_string(&golden_path)
            .unwrap_or_else(|e| panic!("read golden {}: {e}", golden_path.display()));
        let actual = format_ok(&input, profile_of_path(&input_path).unwrap(), &name);
        assert_eq!(actual, golden, "`{name}` did not format to its golden");
    }
}

#[test]
fn every_golden_is_a_fixed_point() {
    let goldens = source_files(&test_data("formatted"));
    assert!(!goldens.is_empty(), "the golden corpus must not be empty");
    for golden_path in goldens {
        let name = golden_path
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        let golden = fs::read_to_string(&golden_path).expect("read golden");
        let actual = format_ok(&golden, profile_of_path(&golden_path).unwrap(), &name);
        assert_eq!(actual, golden, "`{name}` is not a fixed point");
    }
}

fn profile_of_path(path: &Path) -> Option<Profile> {
    match path.extension()?.to_str()? {
        "typl" => Some(Profile::Typl),
        "ridl" => Some(Profile::Ridl),
        "rsdl" => Some(Profile::Rsdl),
        _ => None,
    }
}
