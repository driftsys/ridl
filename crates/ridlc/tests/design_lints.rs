use ridl_core::RidlDatabase;
use ridl_core::diag::{Diagnostic, Severity, SourceMap, Span};

const UNIT: &str = "TYPL-222";
const DESIGN_LINTS: &[&str] = &[
    "inconsistent-unit",
    "inconsistent-abbreviation",
    "duplicate-shape",
    "low-cohesion-interface",
    "package-fan-out",
];
const TYPES: &str =
    "type Speed: km/h [0.0..250.0 step 0.5]\ntype SpeedMs: m/s [0.0..100.0 step 0.5]\n";

fn workspace(packages: &[(&str, &str)]) -> ridlc::WorkspaceOutput {
    let dir = tempfile::tempdir().unwrap();
    let members = packages
        .iter()
        .map(|(name, _)| format!("\"{name}\""))
        .collect::<Vec<_>>()
        .join(", ");
    std::fs::write(
        dir.path().join("ridl.toml"),
        format!("[workspace]\nmembers = [{members}]\n"),
    )
    .unwrap();
    for (name, source) in packages {
        let member = dir.path().join(name);
        std::fs::create_dir(&member).unwrap();
        std::fs::write(
            member.join("ridl.toml"),
            format!("[package]\nname = \"{name}\"\nversion = \"1.0.0\"\n"),
        )
        .unwrap();
        std::fs::write(member.join("source.ridl"), source).unwrap();
    }
    let output = ridlc::compile_workspace(&mut RidlDatabase::default(), dir.path()).unwrap();
    assert_no_errors(&output.diagnostics);
    output
}

fn assert_no_errors(diagnostics: &[Diagnostic]) {
    assert!(
        !diagnostics.iter().any(|d| d.severity == Severity::Error),
        "{diagnostics:?}"
    );
}

fn units(diagnostics: &[Diagnostic]) -> Vec<&Diagnostic> {
    diagnostics
        .iter()
        .filter(|d| d.code.as_str() == UNIT)
        .collect()
}

fn text_at(sources: &SourceMap, span: Span) -> &str {
    &sources.text(span.file).unwrap()
        [usize::from(span.range.start())..usize::from(span.range.end())]
}

fn majority_source() -> String {
    format!(
        "package a\n{TYPES}struct First {{ speed: Speed }}\nstruct Second {{ speed: Speed }}\nstruct Third {{ speed: SpeedMs }}\n"
    )
}

#[test]
fn inconsistent_unit_reports_the_minority_site() {
    let source = majority_source();
    let out = workspace(&[("a", &source)]);
    let found = units(&out.diagnostics);
    assert_eq!(found.len(), 1, "{:?}", out.diagnostics);
    let diagnostic = found[0];
    assert_eq!(
        diagnostic.message,
        "`speed` uses `m/s` here; elsewhere `speed` uses `km/h`"
    );
    assert_eq!(diagnostic.severity, Severity::Info);
    assert_eq!(text_at(&out.sources, diagnostic.primary), "speed");
    assert_eq!(
        usize::from(diagnostic.primary.range.start()),
        source.rfind("speed: SpeedMs").unwrap()
    );
    assert_eq!(diagnostic.labels.len(), 1);
    assert_eq!(text_at(&out.sources, diagnostic.labels[0].span), "speed");
    assert_eq!(
        usize::from(diagnostic.labels[0].span.range.start()),
        source.find("speed: Speed }").unwrap()
    );
}

#[test]
fn inconsistent_unit_reports_every_site_on_a_tie() {
    let source = format!(
        "package a\n{TYPES}struct First {{ speed: Speed }}\nstruct Second {{ speed: SpeedMs }}\n"
    );
    let out = workspace(&[("a", &source)]);
    let found = units(&out.diagnostics);
    assert_eq!(found.len(), 2, "{:?}", out.diagnostics);
    for diagnostic in found {
        assert_eq!(text_at(&out.sources, diagnostic.primary), "speed");
        assert_eq!(diagnostic.labels.len(), 1);
        assert_ne!(diagnostic.primary, diagnostic.labels[0].span);
    }
}

#[test]
fn inconsistent_unit_follows_aliases_and_optional_and_skips_collections() {
    let types = format!("package a\n{TYPES}struct First {{ speed: Speed }}\n");
    let source = "package b\nimport a.Speed as Alias\nimport a.SpeedMs\nstruct Second { speed: Alias }\nstruct Third { speed: SpeedMs? }\nstruct Array { speed: [SpeedMs; 2] }\nstruct Map { speed: [integer: SpeedMs; 2] }\nstruct Tuple { speed: (value: SpeedMs) }\ninterface Input { command submit(speed: <SpeedMs>) @[..1s] }\n";
    let out = workspace(&[("a", &types), ("b", source)]);
    let found = units(&out.diagnostics);
    assert_eq!(found.len(), 1, "{:?}", out.diagnostics);
    assert_eq!(
        found[0].message,
        "`speed` uses `m/s` here; elsewhere `speed` uses `km/h`"
    );
    assert_eq!(
        usize::from(found[0].primary.range.start()),
        source.find("speed: SpeedMs?").unwrap()
    );
    assert!(
        out.sources
            .path(found[0].primary.file)
            .unwrap()
            .ends_with("b/source.ridl")
    );
}

#[test]
fn inconsistent_unit_covers_parameters_and_signals() {
    let source = format!(
        "package a\n{TYPES}interface Input {{\n  command submit(speed: Speed) @[..1s]\n  query inspect(speed: Speed): Speed @[..1s]\n  signal speed: SpeedMs @[100ms..1s]\n}}\n"
    );
    let out = workspace(&[("a", &source)]);
    let found = units(&out.diagnostics);
    assert_eq!(found.len(), 1, "{:?}", out.diagnostics);
    assert_eq!(
        usize::from(found[0].primary.range.start()),
        source.find("speed: SpeedMs").unwrap()
    );
    assert_eq!(text_at(&out.sources, found[0].labels[0].span), "speed");
}

#[test]
fn inconsistent_unit_covers_inline_service_event_and_fixed_payloads() {
    let source = format!(
        "package a\n{TYPES}struct First {{ speed: Speed }}\nservice a.samples {{\n  event speed: Speed @[100ms..1s]\n}}\nservice a.limit {{\n  fixed speed: SpeedMs\n}}\n"
    );
    let out = workspace(&[("a", &source)]);
    let found = units(&out.diagnostics);
    assert_eq!(found.len(), 1, "{:?}", out.diagnostics);
    assert_eq!(
        usize::from(found[0].primary.range.start()),
        source.find("speed: SpeedMs").unwrap()
    );
}

#[test]
fn design_lints_are_silent_without_applicable_findings() {
    let source =
        "package a\nstruct Entry { value: integer [0..5] }\nstruct Choice { enabled: boolean }\n";
    let out = workspace(&[("a", source)]);
    let single = ridlc::check_source("a.ridl", source);
    assert_no_errors(&single.diagnostics);
    for diagnostics in [&out.diagnostics, &single.diagnostics] {
        assert!(
            !diagnostics.iter().any(|d| DESIGN_LINTS
                .iter()
                .any(|name| ridl_core::lint::lint_by_name(name)
                    .is_some_and(|entry| entry.code == d.code))),
            "{diagnostics:?}"
        );
    }
}

#[test]
fn check_source_reports_inconsistent_unit() {
    let source = majority_source();
    let out = ridlc::check_source("a.ridl", &source);
    assert_no_errors(&out.diagnostics);
    let found = units(&out.diagnostics);
    assert_eq!(found.len(), 1, "{:?}", out.diagnostics);
    assert_eq!(found[0].severity, Severity::Info);
    assert_eq!(
        found[0].message,
        "`speed` uses `m/s` here; elsewhere `speed` uses `km/h`"
    );
    assert_eq!(out.sources.path(found[0].primary.file), Some("a.ridl"));
    assert_eq!(text_at(&out.sources, found[0].primary), "speed");
    assert_eq!(
        usize::from(found[0].primary.range.start()),
        source.rfind("speed: SpeedMs").unwrap()
    );
}

#[test]
fn shared_pass_reads_inline_scalar_backing_in_the_current_render_map() {
    use ridl_core::db::InputFile;
    use ridl_core::package::{Package, PackageOrigin, Workspace};
    use ridl_ir::v2::{backing, decl, field_type, struct_member};
    use std::collections::BTreeMap;

    // The current source grammar admits inline constrained primitives. The IR
    // backing also admits units, tested here without inventing new syntax.
    let source = format!(
        "package a\n{TYPES}struct First {{ speed: Speed }}\nstruct Second {{ speed: Speed }}\nstruct Third {{ speed: float [0.0..1.0 step 0.1] }}\n"
    );
    let mut db = RidlDatabase::default();
    let std = ridl_core::std_package(&mut db);
    let input = InputFile::new(&db, "a.ridl".to_string(), source.clone());
    let package = Package::new(
        &db,
        "a".to_string(),
        vec![input],
        PackageOrigin::WorkspaceMember,
        BTreeMap::new(),
        None,
        None,
    );
    let workspace = Workspace::new(&db, vec![package], BTreeMap::new());
    let resolution = ridl_sem::resolve_package(&db, workspace, package, std);
    let mut checked = ridl_sem::check_package(&db, workspace, package, std);
    assert_no_errors(&checked.diagnostics);
    let third = checked
        .ir
        .decls
        .iter_mut()
        .find(|d| d.name == "Third")
        .unwrap();
    let Some(decl::Kind::StructDef(third)) = &mut third.kind else {
        panic!("struct fixture")
    };
    let Some(struct_member::Member::Field(field)) = &mut third.members[0].member else {
        panic!("field fixture")
    };
    let ty = field.r#type.as_mut().unwrap();
    let Some(field_type::Kind::InlineScalar(scalar)) = &mut ty.kind else {
        panic!("inline scalar fixture")
    };
    scalar.backing.as_mut().unwrap().kind = Some(backing::Kind::Unit("m/s".to_string()));
    ty.optional = true;
    let std_ir = ridl_sem::check_package(&db, workspace, std, std).ir;
    let mut sources = SourceMap::new();
    // An unrelated file must not displace the shared pass's diagnostic span.
    sources.file_id("earlier.ridl", "package earlier\n");
    let source_id = sources.file_id("a.ridl", &source);
    let diagnostics = ridlc::check_design_lints(
        &db,
        &[package],
        &[checked],
        &[resolution],
        &std_ir,
        None,
        &mut sources,
    );
    let found = units(&diagnostics);
    assert_eq!(found.len(), 1, "{diagnostics:?}");
    assert_eq!(found[0].primary.file, source_id);
    assert_eq!(
        found[0].message,
        "`speed` uses `m/s` here; elsewhere `speed` uses `km/h`"
    );
    assert_eq!(
        usize::from(found[0].primary.range.start()),
        source.find("speed: float").unwrap()
    );
}

#[test]
fn inconsistent_unit_reports_all_other_units_and_ignores_case_differences() {
    let source = format!(
        "package a\n{TYPES}type SpeedCm: cm/s [0.0..100.0 step 0.5]\nstruct First {{ speed: Speed }}\nstruct Second {{ speed: Speed }}\nstruct Third {{ speed: SpeedMs }}\nstruct Fourth {{ speed: SpeedCm }}\nstruct Different {{ Speed: SpeedCm }}\n"
    );
    let out = workspace(&[("a", &source)]);
    let found = units(&out.diagnostics);
    assert_eq!(found.len(), 2, "{:?}", out.diagnostics);
    for diagnostic in found {
        assert_eq!(diagnostic.labels.len(), 2);
        assert_eq!(text_at(&out.sources, diagnostic.primary), "speed");
        assert!(diagnostic.message.contains("`km/h`"));
    }
}
