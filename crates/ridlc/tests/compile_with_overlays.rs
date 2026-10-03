use ridl_core::{Overlay, RidlDatabase, Severity};
use ridlc::compile_workspace_with;
use std::fs;

fn package() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("ridl.toml"),
        "[package]\nname = \"p\"\nversion = \"1.0.0\"\n",
    )
    .unwrap();
    let path = dir.path().join("a.typl");
    fs::write(&path, "package p\ntype A: integer [0..1]\n").unwrap();
    (dir, path)
}

#[test]
fn an_overlay_reaches_the_checked_ir() {
    let (dir, path) = package();
    let mut db = RidlDatabase::default();
    let output = compile_workspace_with(
        &mut db,
        dir.path(),
        &[Overlay {
            path,
            text: "package p\ntype A: integer [0..1]\ntype B: integer [0..1]\n".into(),
        }],
    )
    .unwrap();
    let names: Vec<_> = output.checked[0]
        .ir
        .decls
        .iter()
        .map(|d| d.name.as_str())
        .collect();
    assert_eq!(names, ["A", "B"]);
}

#[test]
fn an_overlay_error_is_a_diagnostic_with_a_span_in_the_overlay() {
    let (dir, path) = package();
    let mut db = RidlDatabase::default();
    let output = compile_workspace_with(
        &mut db,
        dir.path(),
        &[Overlay {
            path: path.clone(),
            text: "package p\n\n\n\nstruct Broken {\n  value: Missing\n}\n".into(),
        }],
    )
    .unwrap();
    assert_eq!(
        output
            .diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Error)
            .count(),
        1
    );
    let json = ridl_core::diag::to_json(&output.diagnostics, &output.sources);
    assert_eq!(json[0].span.path, path.to_string_lossy());
    assert_eq!(json[0].span.start.line, 6);
}

#[test]
fn imports_are_positional_with_checked() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("ridl.toml"),
        "[workspace]\nmembers = [\"a\", \"b\"]\n",
    )
    .unwrap();
    for name in ["a", "b"] {
        fs::create_dir(dir.path().join(name)).unwrap();
        let imports = if name == "b" {
            "\n[imports]\next = \"https://example.invalid/ext.git\"\n"
        } else {
            ""
        };
        fs::write(
            dir.path().join(name).join("ridl.toml"),
            format!("[package]\nname = \"{name}\"\nversion = \"1.0.0\"\n{imports}"),
        )
        .unwrap();
        fs::write(
            dir.path().join(name).join("a.typl"),
            format!("package {name}\ntype A: integer [0..1]\n"),
        )
        .unwrap();
    }
    let output = compile_workspace_with(&mut RidlDatabase::default(), dir.path(), &[]).unwrap();
    assert_eq!(output.imports.len(), output.checked.len());
    for (i, checked) in output.checked.iter().enumerate() {
        if checked.ir.name == "b" {
            assert_eq!(
                output.imports[i].get("ext").unwrap(),
                "https://example.invalid/ext.git"
            );
        } else {
            assert!(output.imports[i].is_empty());
        }
    }
}
