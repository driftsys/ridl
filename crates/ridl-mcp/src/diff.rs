//! The CLI diff contract exposed without writes or remote fetches.
use crate::{snapshot::ToolError, types::OverlayInput};
use rmcp::schemars::JsonSchema;
use serde::Deserialize;
use std::path::Path;
#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
pub struct DiffInput {
    /// The old side as an .ir.json file, a snapshot directory such as <root>/.ridl/baseline, or a source path.
    pub old: String,
    /// The new side as an .ir.json file, a snapshot directory such as <root>/.ridl/baseline, or a source path.
    pub new: String,
    /// Optional unsaved source files applied to the new source side only.
    pub overlays: Option<Vec<OverlayInput>>,
    /// Optional encodings the change is judged for, as a manifest's `[backend.<name>] encodings` names them (flatbuffers, proto3, podbuf). podbuf, flatbuffers and proto3 have rules of their own: with podbuf, a podbuf_inline_append, podbuf_width_change or podbuf_remote_inline_growth change is breaking; without it, compatible. With flatbuffers, a flatbuffers_width_change change is breaking; without it, compatible. With proto3, a proto3_width_change change is breaking; without it, compatible. A strict_enum_append change is breaking for every list. The same as `ridl diff --encoding`.
    pub encodings: Option<Vec<String>>,
}
pub fn diff(input: &DiffInput) -> Result<serde_json::Value, ToolError> {
    let encodings = input.encodings.as_deref().unwrap_or_default();
    if let Some(unknown) = encodings
        .iter()
        .find(|name| !ridl_core::manifest::ENCODING_NAMES.contains(&name.as_str()))
    {
        return Err(ToolError::Request(format!(
            "unknown encoding `{unknown}`; the encodings are {}",
            ridl_core::manifest::ENCODING_NAMES.join(", ")
        )));
    }
    let encodings: Vec<&str> = encodings.iter().map(String::as_str).collect();
    let mut db = ridl_core::RidlDatabase::default();
    let load_error = |side: &str, error: ridlc::DiffSideError| match error {
        ridlc::DiffSideError::Compile {
            diagnostics,
            sources,
        } => ToolError::WithData {
            message: format!("the {side} side does not compile"),
            data: serde_json::json!({ "diagnostics": ridl_core::diag::to_json(&diagnostics, &sources) }),
        },
        error => ToolError::Request(error.to_string()),
    };
    let old = ridlc::load_diff_side(&mut db, Path::new(&input.old), &[])
        .map_err(|error| load_error("old", error))?;
    let overlays = input
        .overlays
        .as_deref()
        .unwrap_or_default()
        .iter()
        .map(|overlay| ridl_core::Overlay {
            path: overlay.path.clone().into(),
            text: overlay.source.clone(),
        })
        .collect::<Vec<_>>();
    let new = ridlc::load_diff_side(&mut db, Path::new(&input.new), &overlays)
        .map_err(|error| load_error("new", error))?;
    // Historical resolution stays independent of the candidate's remotes.
    let report = ridl_diff::diff_workspaces_with_contexts(
        &old.packages,
        old.system.as_ref(),
        &new.packages,
        new.system.as_ref(),
        &ridlc::diff_side_context(&old),
        &ridlc::diff_side_context(&new),
        &encodings,
    ).map_err(|error| {
        let mut message = format!("cannot establish podbuf compatibility: {error}\n\
Repair context only from the exact historical local snapshots and verified historical remote pins: ridl baseline HISTORICAL_SOURCE --repair-context --out BASELINE.\n\
To start a new history, use ridl baseline CURRENT_SOURCE --reset-history --out BASELINE; reset preserves the prior baseline and claims no earlier compatibility. BASELINE is the affected snapshot input named by the old or new context issue.");
        if error.issues.iter().any(|issue| issue.side == ridl_diff::ContextSide::New) {
            message.push_str("\nFor missing or unusable candidate evidence, compare its source path or an enriched snapshot directory.");
        }
        ToolError::Request(message)
    })?;
    Ok(serde_json::from_str(&ridl_diff::render_json(&report))
        .expect("render_json writes valid JSON"))
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::{ToolError, tests::fixture};
    use crate::types::OverlayInput;
    #[test]
    fn diff_matches_the_cli() {
        let old = fixture("ws");
        let new = fixture("ws-v2");
        let mut db = ridl_core::RidlDatabase::default();
        let a = ridlc::load_diff_side(&mut db, std::path::Path::new(&old), &[]).unwrap();
        let b = ridlc::load_diff_side(&mut db, std::path::Path::new(&new), &[]).unwrap();
        let report = ridl_diff::diff_workspaces(
            &a.packages,
            a.system.as_ref(),
            &b.packages,
            b.system.as_ref(),
            &[ridlc::std_ir()],
        );
        let expected: serde_json::Value =
            serde_json::from_str(&ridl_diff::render_json(&report)).unwrap();
        let result = diff(&DiffInput {
            old,
            new,
            overlays: None,
            encodings: None,
        })
        .unwrap();
        assert_eq!(result, expected);
        assert_eq!(result["verdict"], "breaking");
    }
    #[test]
    fn diff_with_an_overlay_on_new() {
        let root = fixture("ws");
        let path = format!("{root}/a/a.ridl");
        let source = std::fs::read_to_string(&path)
            .unwrap()
            .replace("FAIL = 2", "FAIL = 2\n  DEAD = 3");
        let result = diff(&DiffInput {
            old: root.clone(),
            new: root,
            overlays: Some(vec![OverlayInput { path, source }]),
            encodings: None,
        })
        .unwrap();
        // The enum is strict, so the appended value is breaking.
        assert_eq!(result["verdict"], "breaking");
        assert!(
            result.to_string().contains("strict_enum_append"),
            "{result}"
        );
    }
    #[test]
    fn diff_with_a_missing_old_side_is_a_tool_error() {
        let old = format!("{}/.ridl/baseline", fixture("ws"));
        let expected = format!("{old}: `{old}` does not exist");
        match diff(&DiffInput {
            old,
            new: fixture("ws"),
            overlays: None,
            encodings: None,
        })
        .unwrap_err()
        {
            ToolError::Request(message) => assert_eq!(message, expected),
            _ => panic!("expected a request error"),
        }
    }
    #[test]
    fn diff_with_a_side_that_does_not_compile_carries_diagnostics() {
        match diff(&DiffInput {
            old: fixture("ws"),
            new: fixture("ws-diag"),
            overlays: None,
            encodings: None,
        })
        .unwrap_err()
        {
            ToolError::WithData { message, data } => {
                assert_eq!(message, "the new side does not compile");
                let path = fixture("ws-diag");
                let compiled = ridlc::compile_workspace(
                    &mut ridl_core::RidlDatabase::default(),
                    std::path::Path::new(&path),
                )
                .unwrap();
                assert_eq!(
                    data["diagnostics"],
                    serde_json::to_value(ridl_core::diag::to_json(
                        &compiled.diagnostics,
                        &compiled.sources,
                    ))
                    .unwrap()
                );
                let all_diagnostics = data["diagnostics"].as_array().unwrap();
                let missing_docs_code = ridl_core::lint::lint_by_name("missing-docs")
                    .unwrap()
                    .code
                    .as_str();
                let expected_codes = std::iter::once("TYPL-103")
                    .chain(std::iter::repeat_n(missing_docs_code, 15))
                    .chain(std::iter::once("TYPL-011"))
                    .chain(std::iter::repeat_n(missing_docs_code, 8))
                    .collect::<Vec<_>>();
                assert_eq!(
                    all_diagnostics
                        .iter()
                        .map(|diagnostic| diagnostic["code"].as_str().unwrap())
                        .collect::<Vec<_>>(),
                    expected_codes
                );
                assert!(
                    all_diagnostics
                        .iter()
                        .filter(|diagnostic| diagnostic["lint"] == "missing-docs")
                        .all(|diagnostic| diagnostic["severity"] == "warning")
                );
                // Keep the incoming fixture allowance while checking every other diagnostic exactly.
                let exact_diagnostics = all_diagnostics
                    .iter()
                    .filter(|diagnostic| diagnostic["lint"] != "missing-docs")
                    .collect::<Vec<_>>();
                assert_eq!(
                    exact_diagnostics
                        .iter()
                        .map(|d| d["code"].as_str().unwrap())
                        .collect::<Vec<_>>(),
                    ["TYPL-103", "TYPL-011"]
                );
                assert_eq!(
                    serde_json::to_value(exact_diagnostics).unwrap(),
                    serde_json::json!([
                        {
                            "code": "TYPL-103",
                            "severity": "warning",
                            "lint": "unbounded-length",
                            "message": "`string` without explicit bounds; the default `[0..256]` applies",
                            "span": {
                                "path": format!("{}/a/a.ridl", fixture("ws-diag")),
                                "start": {
                                    "line": 25,
                                    "column": 11
                                },
                                "end": {
                                    "line": 25,
                                    "column": 17
                                }
                            },
                            "labels": [],
                            "fixes": []
                        },
                        {
                            "code": "TYPL-011",
                            "severity": "error",
                            "message": "unknown type name `Missing`",
                            "span": {
                                "path": format!("{}/b/b.ridl", fixture("ws-diag")),
                                "start": {
                                    "line": 18,
                                    "column": 25
                                },
                                "end": {
                                    "line": 18,
                                    "column": 32
                                }
                            },
                            "labels": [],
                            "fixes": []
                        }
                    ])
                );
            }
            _ => panic!("expected diagnostics"),
        }
    }

    /// An append that grows a struct another struct holds inline is
    /// `podbuf_inline_append`: compatible with no `encodings`, breaking with
    /// `["podbuf"]`. The appended eight-byte integer grows the 8-byte head.
    #[test]
    fn diff_with_podbuf_makes_an_inline_append_breaking() {
        let copy = crate::snapshot::tests::TempWorkspace::copy("ws");
        let path = copy.0.join("a/a.ridl");
        let text = std::fs::read_to_string(&path).unwrap().replace(
            "union Outcome {",
            "struct Pair {\n  first: Reading\n  second: Reading\n}\n\nunion Outcome {",
        );
        std::fs::write(&path, &text).unwrap();
        let appended = text.replace(
            "  health: Health\n}",
            "  health: Health\n  quality: integer [0..9223372036854775807]\n}",
        );
        let input = |encodings: Option<Vec<String>>| DiffInput {
            old: copy.0.to_str().unwrap().into(),
            new: copy.0.to_str().unwrap().into(),
            overlays: Some(vec![OverlayInput {
                path: path.to_str().unwrap().into(),
                source: appended.clone(),
            }]),
            encodings,
        };
        let without = diff(&input(None)).unwrap();
        assert_eq!(without["verdict"], "compatible");
        let with = diff(&input(Some(vec!["podbuf".into()]))).unwrap();
        assert_eq!(with["verdict"], "breaking");
        let podbuf = with["changes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|change| change["category"] == "podbuf_inline_append")
            .expect("the podbuf line is reported");
        assert_eq!(podbuf["verdict"], "breaking");
        assert_eq!(podbuf["path"], "fx.a/Reading/quality");
        assert_eq!(
            podbuf["after"],
            "nested inline in fx.a/Pair/first, fx.a/Pair/second"
        );
        let flatbuffers_only = diff(&input(Some(vec!["flatbuffers".into()]))).unwrap();
        assert_eq!(flatbuffers_only["verdict"], "compatible");
    }

    #[test]
    fn diff_with_an_unknown_encoding_is_a_tool_error() {
        match diff(&DiffInput {
            old: fixture("ws"),
            new: fixture("ws"),
            overlays: None,
            encodings: Some(vec!["podbuff".into()]),
        })
        .unwrap_err()
        {
            ToolError::Request(message) => assert_eq!(
                message,
                "unknown encoding `podbuff`; the encodings are flatbuffers, proto3, podbuf"
            ),
            _ => panic!("expected a request error"),
        }
    }

    #[test]
    fn diff_uses_std_as_context() {
        let copy = crate::snapshot::tests::TempWorkspace::copy("ws");
        let path = copy.0.join("a/a.ridl");
        let text = std::fs::read_to_string(&path).unwrap();
        std::fs::write(
            &path,
            text.replace(
                "  health: Health\n}",
                "  health: Health\n  stamp: Timestamp\n}",
            ),
        )
        .unwrap();
        let input = DiffInput {
            old: fixture("ws"),
            new: copy.0.to_str().unwrap().into(),
            overlays: None,
            encodings: None,
        };
        let cli = std::process::Command::new("cargo")
            .current_dir(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
            .args([
                "run", "-q", "-p", "ridl-cli", "--locked", "--", "diff", "--format", "json",
                &input.old, &input.new,
            ])
            .output()
            .unwrap();
        assert_eq!(
            cli.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&cli.stderr)
        );
        let expected: serde_json::Value = serde_json::from_slice(&cli.stdout).unwrap();
        assert_eq!(expected["verdict"], "compatible");
        assert_eq!(diff(&input).unwrap(), expected);
    }

    fn remote_source(root: &Path, grown: bool, inline: bool) {
        let vendor = root.join("vendor/dep");
        std::fs::create_dir_all(&vendor).unwrap();
        std::fs::write(
            vendor.join("ridl.toml"),
            "[package]\nname = \"dep\"\nversion = \"1.0.0\"\n",
        )
        .unwrap();
        let extra = if grown {
            "\n second: integer [0..9223372036854775807]"
        } else {
            ""
        };
        std::fs::write(vendor.join("types.typl"), format!("package dep\nstruct Reading {{\n first: integer [0..9223372036854775807]{extra}\n}}\n")).unwrap();
        std::fs::write(root.join("ridl.toml"), "[package]\nname = \"app\"\nversion = \"1.0.0\"\n[imports]\ndep = \"https://example.invalid/dep.tar\"\n").unwrap();
        let payload = if inline { "Reading" } else { "integer" };
        std::fs::write(root.join("app.typl"), format!("package app\nimport dep.Reading\nstruct Sample {{\n reading: {payload}\n sequence: integer [0..9223372036854775807]\n}}\n")).unwrap();
        ridl_core::lock::write_lockfile(
            &root.join("ridl.lock"),
            &ridl_core::lock::Lockfile {
                entries: [(
                    "https://example.invalid/dep.tar".into(),
                    ridl_core::lock::LockEntry {
                        sha256: "a".repeat(64),
                        tree: Some(ridl_core::tree_hash(&vendor).unwrap()),
                    },
                )]
                .into_iter()
                .collect(),
            },
        )
        .unwrap();
    }

    fn remote_baseline(source: &Path, out: &Path, enriched: bool) {
        let output =
            ridlc::compile_workspace(&mut ridl_core::RidlDatabase::default(), source).unwrap();
        assert!(
            !output
                .diagnostics
                .iter()
                .any(|d| d.severity == ridl_core::Severity::Error)
        );
        let (remote, own): (Vec<_>, Vec<_>) = output
            .checked
            .iter()
            .map(|p| &p.ir)
            .partition(|p| ridlc::is_remote(p, &output.remote_units));
        std::fs::create_dir_all(out).unwrap();
        for package in &own {
            std::fs::write(
                out.join(format!("{}.ir.json", package.name)),
                ridl_ir::v2::to_json_pretty(package).unwrap(),
            )
            .unwrap();
        }
        if enriched {
            let context =
                ridlc::baseline_context::capture_context(&own, &remote, &output.remote_units)
                    .unwrap();
            ridlc::baseline_context::write_context(out, &context).unwrap();
        }
    }

    fn remote_input(old: &Path, new: &Path) -> DiffInput {
        DiffInput {
            old: old.to_str().unwrap().into(),
            new: new.to_str().unwrap().into(),
            overlays: None,
            encodings: Some(vec!["podbuf".into()]),
        }
    }

    /// Compare complete file identities, bytes and modification times around reads.
    fn inventory(
        root: &Path,
    ) -> std::collections::BTreeMap<std::path::PathBuf, (Vec<u8>, std::time::SystemTime)> {
        let mut files = std::collections::BTreeMap::new();
        if !root.exists() {
            return files;
        }
        for entry in std::fs::read_dir(root).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                files.extend(inventory(&path));
            } else {
                files.insert(
                    path.clone(),
                    (
                        std::fs::read(&path).unwrap(),
                        std::fs::metadata(path).unwrap().modified().unwrap(),
                    ),
                );
            }
        }
        files
    }

    #[test]
    fn remote_pin_growth_matches_cli_without_writing_inputs() {
        let copy = crate::snapshot::tests::TempWorkspace::copy("ws");
        let old = copy.0.join("remote-old");
        let new = copy.0.join("remote-new");
        remote_source(&old, false, true);
        remote_source(&new, true, true);
        let baseline = copy.0.join("remote-baseline");
        remote_baseline(&old, &baseline, true);
        let before = inventory(&copy.0);
        // SHA-256 of the fixture URL: inspect the cache entry a fetch would write.
        let cache = ridl_core::Cache::user_default()
            .root
            .join("83d8e8e1b5362aa8d63beac38c539cc1aabdb41d0e7257a3fde21269afedfebd");
        let cache_before = inventory(&cache);
        for old in [&old, &baseline] {
            let input = remote_input(old, &new);
            let result = diff(&input).unwrap();
            assert_eq!(result["verdict"], "breaking");
            assert_eq!(
                result["changes"][0]["category"],
                "podbuf_remote_inline_growth"
            );
            assert_eq!(result["changes"][0]["path"], "app/Sample/reading");
            let cli = std::process::Command::new("cargo")
                .current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
                .args([
                    "run",
                    "-q",
                    "-p",
                    "ridl-cli",
                    "--locked",
                    "--offline",
                    "--",
                    "diff",
                    "--format",
                    "json",
                    "--encoding",
                    "podbuf",
                    &input.old,
                    &input.new,
                ])
                .output()
                .unwrap();
            assert_eq!(
                cli.status.code(),
                Some(1),
                "{}",
                String::from_utf8_lossy(&cli.stderr)
            );
            assert_eq!(
                result,
                serde_json::from_slice::<serde_json::Value>(&cli.stdout).unwrap()
            );
            let mut input = input;
            input.encodings = None;
            let compatible = diff(&input).unwrap();
            assert_eq!(compatible["verdict"], "compatible");
            assert_eq!(
                compatible["changes"][0]["category"],
                "podbuf_remote_inline_growth"
            );
        }
        let path = new.join("app.typl");
        let mut input = remote_input(&old, &new);
        input.overlays = Some(vec![OverlayInput {
            path: path.to_str().unwrap().into(),
            source: std::fs::read_to_string(&path)
                .unwrap()
                .replace("struct Sample", "struct Candidate"),
        }]);
        let overlaid = diff(&input).unwrap();
        assert!(
            overlaid["changes"]
                .as_array()
                .unwrap()
                .iter()
                .any(|c| c["path"].as_str().unwrap().contains("Candidate"))
        );
        assert_eq!(before, inventory(&copy.0));
        assert_eq!(cache_before, inventory(&cache));
    }

    #[test]
    fn remote_legacy_context_is_a_tool_error_with_recovery_guidance() {
        let copy = crate::snapshot::tests::TempWorkspace::copy("ws");
        let source = copy.0.join("remote-source");
        remote_source(&source, false, true);
        let legacy = copy.0.join("remote-legacy");
        let enriched = copy.0.join("remote-enriched");
        remote_baseline(&source, &legacy, false);
        remote_baseline(&source, &enriched, true);
        let before = inventory(&copy.0);
        for (old, new, missing_side) in [
            (&legacy, &source, "old context is missing dep.Reading"),
            (&legacy, &enriched, "old context is missing dep.Reading"),
            (&legacy, &legacy, "old context is missing dep.Reading"),
            (&enriched, &legacy, "new context is missing dep.Reading"),
        ] {
            match diff(&remote_input(old, new)).unwrap_err() {
                ToolError::Request(message) => {
                    for expected in [
                        "app/Sample/reading",
                        "dep.Reading",
                        missing_side,
                        "HISTORICAL_SOURCE --repair-context --out",
                        "CURRENT_SOURCE --reset-history --out",
                        "historical",
                        "preserv",
                    ] {
                        assert!(message.contains(expected), "missing {expected}: {message}");
                    }
                    if missing_side.starts_with("new") {
                        assert!(
                            message.contains("candidate")
                                && message.contains("source path")
                                && message.contains("enriched snapshot directory"),
                            "{message}"
                        );
                    }
                }
                _ => panic!("missing evidence must use the existing request error"),
            }
        }
        let mut input = remote_input(&legacy, &source);
        input.encodings = None;
        assert_eq!(diff(&input).unwrap()["verdict"], "identical");
        assert_eq!(
            diff(&remote_input(&enriched, &source)).unwrap()["verdict"],
            "identical"
        );
        assert_eq!(before, inventory(&copy.0));
    }

    #[test]
    fn legacy_context_without_remote_uses_and_malformed_enriched_inputs() {
        let copy = crate::snapshot::tests::TempWorkspace::copy("ws");
        let source = copy.0.join("remote-source");
        remote_source(&source, false, false);
        let baseline = copy.0.join("remote-baseline");
        remote_baseline(&source, &baseline, false);
        assert_eq!(
            diff(&remote_input(&baseline, &source)).unwrap()["verdict"],
            "identical"
        );
        std::fs::write(baseline.join("context.json"), "{broken").unwrap();
        let before = inventory(&copy.0);
        match diff(&remote_input(&baseline, &source)).unwrap_err() {
            ToolError::Request(message) => assert!(message.contains("context.json"), "{message}"),
            _ => panic!("malformed context must be an input error"),
        }
        assert_eq!(before, inventory(&copy.0));
    }
}
