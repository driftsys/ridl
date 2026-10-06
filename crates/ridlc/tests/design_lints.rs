use ridl_core::RidlDatabase;
use ridl_core::TimingDefaults;
use ridl_core::diag::{Diagnostic, Severity, SourceMap, Span};

const UNIT: &str = "TYPL-222";
const COHESION: &str = "RIDL-414";
const DESIGN_LINTS: &[&str] = &[
    "inconsistent-unit",
    "duplicate-shape",
    "low-cohesion-interface",
];
const TYPES: &str =
    "type Speed: km/h [0.0..250.0 step 0.5]\ntype SpeedMs: m/s [0.0..100.0 step 0.5]\n";

fn workspace(packages: &[(&str, &str)]) -> ridlc::WorkspaceOutput {
    let files: Vec<_> = packages
        .iter()
        .map(|(name, source)| (*name, vec![("source.ridl", *source)]))
        .collect();
    workspace_files(&files)
}

fn workspace_files(packages: &[(&str, Vec<(&str, &str)>)]) -> ridlc::WorkspaceOutput {
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
    for (name, files) in packages {
        let member = dir.path().join(name);
        std::fs::create_dir(&member).unwrap();
        std::fs::write(
            member.join("ridl.toml"),
            format!("[package]\nname = \"{name}\"\nversion = \"1.0.0\"\n"),
        )
        .unwrap();
        for (path, source) in files {
            std::fs::write(member.join(path), source).unwrap();
        }
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

fn cohesion(diagnostics: &[Diagnostic]) -> Vec<&Diagnostic> {
    diagnostics
        .iter()
        .filter(|d| d.code.as_str() == COHESION)
        .collect()
}

#[test]
fn low_cohesion_interface_is_reported_with_its_groups() {
    let source = format!("{SEVEN_GROUP_TYPES}interface I {{ {SEVEN_GROUP_MEMBERS} }}\n");
    let out = workspace(&[("a", &source)]);
    let found = cohesion(&out.diagnostics);
    assert_eq!(found.len(), 1, "{:?}", out.diagnostics);
    assert_eq!(
        found[0].message,
        "interface `I` splits into 7 groups of members that share no type: [a, b], [c], [d], [e], [f], [g], [h]"
    );
    assert_eq!(found[0].severity, Severity::Info);
    assert_eq!(
        ridl_core::lint::lint_of(found[0].code).unwrap().lint,
        Some("low-cohesion-interface")
    );
    let start = source.find("I {").unwrap();
    assert_eq!(
        site(&out.sources, found[0].primary),
        ("a/source.ridl".to_string(), start..start + 1)
    );
    assert!(found[0].labels.is_empty());
    assert!(found[0].fixits.is_empty());
}

/// Eight types and the members that split an interface into seven groups,
/// the least strict group count the lint reports.
const SEVEN_GROUP_TYPES: &str = "package a\ntype X: boolean\ntype Y: boolean\ntype Z: boolean\ntype W: boolean\ntype V: boolean\ntype U: boolean\ntype T: boolean\ntype S: boolean\n";
const SEVEN_GROUP_MEMBERS: &str = "command a(x: X) @[..1s] command b(x: X, y: Y) @[..1s] command c(z: Z) @[..1s] command d(w: W) @[..1s] command e(v: V) @[..1s] command f(u: U) @[..1s] command g(t: T) @[..1s] command h(s: S) @[..1s] command reset() @[..1s]";

#[test]
fn an_interface_below_the_group_threshold_is_not_reported() {
    // Six groups: the seventh member `h` is left out.
    let members = SEVEN_GROUP_MEMBERS.replace("command h(s: S) @[..1s] ", "");
    let source = format!("{SEVEN_GROUP_TYPES}interface I {{ {members} }}\n");
    let out = workspace(&[("a", &source)]);
    assert_eq!(
        ridlc::cohesion_groups(&out.checked[0].ir, &out.checked[0].ir.interfaces[0]).len(),
        6
    );
    assert!(
        cohesion(&out.diagnostics).is_empty(),
        "{:?}",
        out.diagnostics
    );
}

#[test]
fn a_cohesive_interface_is_not_reported() {
    let source = "package a\ntype X: boolean\ninterface I { command a(x: X) @[..1s] command b(x: X) @[..1s] command reset() @[..1s] }\n";
    let out = workspace(&[("a", source)]);
    assert!(cohesion(&out.diagnostics).is_empty());
}

fn groups(source: &str) -> Vec<Vec<String>> {
    let out = workspace(&[("a", source)]);
    let pkg = &out.checked[0].ir;
    ridlc::cohesion_groups(pkg, &pkg.interfaces[0])
}

#[test]
fn cohesion_groups_link_members_that_share_a_type() {
    let source = "package a\ntype X: boolean\ntype Y: boolean\ntype Z: boolean\ninterface I { command a(x: X) @[..1s] command b(x: X, y: Y) @[..1s] command c(z: Z) @[..1s] command reset() @[..1s] }\n";
    assert_eq!(groups(source), vec![vec!["a", "b"], vec!["c"]]);
}

#[test]
fn cohesion_groups_follow_first_member_source_order() {
    let source = "package a\ntype X: boolean\ntype Z: boolean\ninterface I { command z(x: X) @[..1s] command y(x: X) @[..1s] command a(z: Z) @[..1s] command reset() @[..1s] }\n";
    assert_eq!(groups(source), vec![vec!["y", "z"], vec!["a"]]);
}

#[test]
fn cohesion_groups_merge_transitively_through_a_later_member() {
    let source = "package a\ntype X: boolean\ntype Y: boolean\ntype Z: boolean\ninterface I { command z(x: X) @[..1s] command separate(z: Z) @[..1s] command a(y: Y) @[..1s] command bridge(x: X, y: Y) @[..1s] }\n";
    assert_eq!(
        groups(source),
        vec![vec!["a", "bridge", "z"], vec!["separate"]]
    );
}

#[test]
fn cohesion_groups_cover_payloads_parameters_returns_and_fallible_arms() {
    let source = "package a\ntype X: boolean\ntype Y: boolean\nerror enum E { FAILED = 0 }\ninterface I { signal status: X @[1s..2s] event changed: X @[1s..2s] fixed initial: X command send(value: X) @[..1s] query read(): X @[..1s] query attempt(): Y | E @[..1s] query retry(): X | E @[..1s] command accept(value: Y) @[..1s] }\n";
    assert_eq!(
        groups(source),
        vec![vec![
            "accept", "attempt", "changed", "initial", "read", "retry", "send", "status"
        ]]
    );
}

#[test]
fn cohesion_groups_keep_named_types_in_anonymous_containers() {
    let source = "package a\ntype X: boolean\ntype Y: boolean\ninterface I { command upload(values: <X>) @[..1s] query download(): <X> @[..1s] fixed initial: [X; 2] query read(): (first: X, second: Y) @[..1s] command send(value: Y) @[..1s] }\n";
    assert_eq!(
        groups(source),
        vec![vec!["download", "initial", "read", "send", "upload"]]
    );
}

#[test]
fn cohesion_groups_link_map_keys_values_and_query_parameters() {
    let source = "package a\ntype Key: string [1..8]\ntype X: boolean\ntype Y: boolean\ninterface I { command key(value: Key) @[..1s] command value(value: X) @[..1s] query read(): (items: [Key: X; 2]) @[..1s] query connect(request: Y): (maybe: X?) @[..1s] command send(value: Y) @[..1s] query raw(): (flag: boolean, count: integer [0..9]) @[..1s] }\n";
    assert_eq!(
        groups(source),
        vec![vec!["connect", "key", "read", "send", "value"]]
    );
}

#[test]
fn cohesion_groups_use_direct_nominal_references_without_expanding_definitions() {
    let source = "package a\ntype X: boolean\nstruct First { value: X }\nstruct Second { value: X }\ninterface I { command a(value: First) @[..1s] command b(value: Second) @[..1s] command c(value: X) @[..1s] }\n";
    assert_eq!(groups(source), vec![vec!["a"], vec!["b"], vec!["c"]]);
}

#[test]
fn cohesion_groups_qualify_local_types_and_preserve_import_identity() {
    let a = "package a\ntype X: boolean\n";
    let b = "package b\nimport a.X as Remote\ntype X: boolean\ninterface I { command local(value: X) @[..1s] command remote(value: Remote) @[..1s] command again(value: a.X) @[..1s] }\n";
    let out = workspace(&[("a", a), ("b", b)]);
    let pkg = &out.checked.iter().find(|p| p.ir.name == "b").unwrap().ir;
    assert_eq!(
        ridlc::cohesion_groups(pkg, &pkg.interfaces[0]),
        vec![vec!["local"], vec!["again", "remote"]]
    );
    let mut iface = pkg.interfaces[0].clone();
    let ridl_ir::v2::decl::Kind::CommandDef(command) = iface.interactions[0].kind.as_mut().unwrap()
    else {
        panic!("command")
    };
    command.params[0].r#type.as_mut().unwrap().kind =
        Some(ridl_ir::v2::field_type::Kind::Named("b.X".into()));
    iface.interactions[1] = pkg.interfaces[0].interactions[0].clone();
    iface.interactions[1].name = "bare".into();
    assert_eq!(
        ridlc::cohesion_groups(pkg, &iface),
        vec![vec!["bare", "local"], vec!["again"]]
    );
}

#[test]
fn cohesion_groups_exclude_standard_types_and_members_without_named_types() {
    let source = "package a\ntype X: boolean\ntype Y: boolean\ninterface I { command a(value: X, delay: Duration) @[..1s] command b(value: Y, delay: Duration) @[..1s] command wait(delay: Duration) @[..1s] command reset() @[..1s] command raw(data: <bytes>) @[..1s] reserved old }\n";
    assert_eq!(groups(source), vec![vec!["a"], vec!["b"]]);
    let empty = "package a\ninterface I { command reset() @[..1s] command wait(delay: Duration) @[..1s] command raw(data: <bytes>) @[..1s] }\n";
    assert!(groups(empty).is_empty());
    let out = workspace(&[("a", empty)]);
    assert!(cohesion(&out.diagnostics).is_empty());
}

#[test]
fn low_cohesion_interface_excludes_standard_packages_and_service_inline_shapes() {
    use ridl_ir::v2::{decl, field_type};

    // Seven singleton groups in each shape, so only the exclusions keep them silent.
    let types = "type A: boolean\ntype B: boolean\ntype C: boolean\ntype D: boolean\ntype E: boolean\ntype F: boolean\ntype G: boolean\n";
    let members = "command a(value: A) @[..1s] command b(value: B) @[..1s] command c(value: C) @[..1s] command d(value: D) @[..1s] command e(value: E) @[..1s] command f(value: F) @[..1s] command g(value: G) @[..1s]";
    let standard = format!("package ridl.std\n{types}interface I {{ {members} }}\n");
    let user = format!("package a\n{types}service a.example {{ {members} }}\n");
    let (diagnostics, _) =
        design_source_set_with(&[("ridl.std", &standard), ("a", &user)], |checked| {
            let pkg = &mut checked
                .iter_mut()
                .find(|package| package.ir.name == "ridl.std")
                .unwrap()
                .ir;
            for (member, name) in pkg.interfaces[0]
                .interactions
                .iter_mut()
                .zip(["a.A", "a.B", "a.C", "a.D", "a.E", "a.F", "a.G"])
            {
                let Some(decl::Kind::CommandDef(command)) = &mut member.kind else {
                    panic!("command")
                };
                command.params[0].r#type.as_mut().unwrap().kind =
                    Some(field_type::Kind::Named(name.into()));
            }
            assert_eq!(
                ridlc::cohesion_groups(pkg, &pkg.interfaces[0]),
                ["a", "b", "c", "d", "e", "f", "g"].map(|member| vec![member])
            );
        });
    assert!(cohesion(&diagnostics).is_empty(), "{diagnostics:?}");
}

#[test]
fn cohesion_groups_exclude_only_the_exact_standard_type_owner() {
    let source = "package ridl.std.extra\ntype X: boolean\ntype Y: boolean\ninterface I { command a(value: X, delay: Duration) @[..1s] command b(value: X) @[..1s] command c(value: Y, delay: Duration) @[..1s] command wait(delay: Duration) @[..1s] }\n";
    let out = workspace(&[("ridl.std.extra", source)]);
    let pkg = &out.checked[0].ir;
    assert_eq!(
        ridlc::cohesion_groups(pkg, &pkg.interfaces[0]),
        vec![vec!["a", "b"], vec!["c"]]
    );
}

fn text_at(sources: &SourceMap, span: Span) -> &str {
    &sources.text(span.file).unwrap()
        [usize::from(span.range.start())..usize::from(span.range.end())]
}

fn site(sources: &SourceMap, span: Span) -> (String, std::ops::Range<usize>) {
    let path = std::path::Path::new(sources.path(span.file).unwrap());
    let relative = format!(
        "{}/{}",
        path.parent()
            .unwrap()
            .file_name()
            .unwrap()
            .to_str()
            .unwrap(),
        path.file_name().unwrap().to_str().unwrap(),
    );
    (
        relative,
        usize::from(span.range.start())..usize::from(span.range.end()),
    )
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
    let first = source.find("speed: Speed }").unwrap();
    let second = source.find("speed: SpeedMs }").unwrap();
    let actual: Vec<_> = found
        .iter()
        .map(|diagnostic| {
            (
                diagnostic.message.as_str(),
                site(&out.sources, diagnostic.primary),
            )
        })
        .collect();
    assert_eq!(
        actual,
        vec![
            (
                "`speed` uses `km/h` here; elsewhere `speed` uses `m/s`",
                ("a/source.ridl".to_string(), first..first + 5)
            ),
            (
                "`speed` uses `m/s` here; elsewhere `speed` uses `km/h`",
                ("a/source.ridl".to_string(), second..second + 5)
            ),
        ]
    );
    let labels: Vec<_> = found
        .iter()
        .map(|diagnostic| {
            diagnostic
                .labels
                .iter()
                .map(|label| (label.message.as_str(), site(&out.sources, label.span)))
                .collect::<Vec<_>>()
        })
        .collect();
    assert_eq!(
        labels,
        vec![
            vec![(
                "`speed` uses `m/s` here",
                ("a/source.ridl".to_string(), second..second + 5)
            )],
            vec![(
                "`speed` uses `km/h` here",
                ("a/source.ridl".to_string(), first..first + 5)
            )],
        ]
    );
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
        TimingDefaults::default(),
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
    let mut sources = SourceMap::new();
    // An unrelated file must not displace the shared pass's diagnostic span.
    sources.file_id("earlier.ridl", "package earlier\n");
    let source_id = sources.file_id("a.ridl", &source);
    let diagnostics =
        ridlc::check_design_lints(&db, &[package], &[checked], &[resolution], &mut sources);
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
    let first = source.find("speed: Speed }").unwrap();
    let third = source.find("speed: SpeedMs }").unwrap();
    let fourth = source.find("speed: SpeedCm }").unwrap();
    let actual: Vec<_> = found
        .iter()
        .map(|diagnostic| {
            (
                diagnostic.message.as_str(),
                site(&out.sources, diagnostic.primary),
                diagnostic
                    .labels
                    .iter()
                    .map(|label| (label.message.as_str(), site(&out.sources, label.span)))
                    .collect::<Vec<_>>(),
            )
        })
        .collect();
    assert_eq!(
        actual,
        vec![
            (
                "`speed` uses `cm/s` here; elsewhere `speed` uses `km/h`, `m/s`",
                ("a/source.ridl".to_string(), fourth..fourth + 5),
                vec![
                    (
                        "`speed` uses `km/h` here",
                        ("a/source.ridl".to_string(), first..first + 5)
                    ),
                    (
                        "`speed` uses `m/s` here",
                        ("a/source.ridl".to_string(), third..third + 5)
                    ),
                ]
            ),
            (
                "`speed` uses `m/s` here; elsewhere `speed` uses `cm/s`, `km/h`",
                ("a/source.ridl".to_string(), third..third + 5),
                vec![
                    (
                        "`speed` uses `cm/s` here",
                        ("a/source.ridl".to_string(), fourth..fourth + 5)
                    ),
                    (
                        "`speed` uses `km/h` here",
                        ("a/source.ridl".to_string(), first..first + 5)
                    ),
                ]
            ),
        ]
    );
}

#[test]
fn inconsistent_unit_skips_same_named_primitive_sites() {
    let source = "package a\ntype Speed: km/h [0.0..250.0 step 0.5]\nstruct Typed { speed: Speed }\nstruct Primitive { speed: float }\nstruct Constrained { speed: float [0.0..100.0 step 0.5] }\n";
    let out = workspace(&[("a", source)]);
    assert!(units(&out.diagnostics).is_empty(), "{:?}", out.diagnostics);
    let single = ridlc::check_source("a.ridl", source);
    assert_no_errors(&single.diagnostics);
    assert!(
        units(&single.diagnostics).is_empty(),
        "{:?}",
        single.diagnostics
    );
}

#[test]
fn shared_pass_excludes_standard_package_sites_from_unit_counts() {
    use ridl_core::db::InputFile;
    use ridl_core::package::{Package, PackageOrigin, Workspace};
    use std::collections::BTreeMap;

    let mut db = RidlDatabase::default();
    let std = ridl_core::std_package(&mut db);
    let make_package = |name: &str, source: String| {
        let input = InputFile::new(&db, format!("{name}/source.ridl"), source);
        Package::new(
            &db,
            name.to_string(),
            vec![input],
            PackageOrigin::WorkspaceMember,
            BTreeMap::new(),
            TimingDefaults::default(),
            None,
        )
    };
    let user = make_package("a", majority_source());
    // A controlled standard-package input adds two m/s sites. Counting them
    // would reverse the user's majority and change its findings.
    let standard = make_package(
        "ridl.std",
        format!(
            "package ridl.std\n{TYPES}struct First {{ speed: SpeedMs }}\nstruct Second {{ speed: SpeedMs }}\n"
        ),
    );
    let packages = [user, standard];
    let workspace = Workspace::new(&db, packages.to_vec(), BTreeMap::new());
    let resolutions: Vec<_> = packages
        .iter()
        .map(|package| ridl_sem::resolve_package(&db, workspace, *package, std))
        .collect();
    let checked: Vec<_> = packages
        .iter()
        .map(|package| ridl_sem::check_package(&db, workspace, *package, std))
        .collect();
    for package in &checked {
        assert_no_errors(&package.diagnostics);
    }
    let mut sources = SourceMap::new();
    let diagnostics =
        ridlc::check_design_lints(&db, &packages, &checked, &resolutions, &mut sources);
    let found = units(&diagnostics);
    assert_eq!(found.len(), 1, "{diagnostics:?}");
    assert_eq!(
        found[0].message,
        "`speed` uses `m/s` here; elsewhere `speed` uses `km/h`"
    );
    let minority = majority_source().find("speed: SpeedMs").unwrap();
    assert_eq!(
        site(&sources, found[0].primary),
        ("a/source.ridl".to_string(), minority..minority + 5)
    );
    assert!(
        found[0]
            .labels
            .iter()
            .all(|label| sources.path(label.span.file) == Some("a/source.ridl"))
    );
}

fn design_source_set(packages: &[(&str, &str)]) -> (Vec<Diagnostic>, SourceMap) {
    design_source_set_with(packages, |_| {})
}

fn design_source_set_with(
    packages: &[(&str, &str)],
    amend: impl FnOnce(&mut [ridl_sem::CheckedPackage]),
) -> (Vec<Diagnostic>, SourceMap) {
    use ridl_core::db::InputFile;
    use ridl_core::package::{Package, PackageOrigin, Workspace};
    use std::collections::BTreeMap;

    let mut db = RidlDatabase::default();
    let std = ridl_core::std_package(&mut db);
    let packages: Vec<_> = packages
        .iter()
        .map(|(name, source)| {
            let input = InputFile::new(&db, format!("{name}/source.ridl"), source.to_string());
            Package::new(
                &db,
                name.to_string(),
                vec![input],
                PackageOrigin::WorkspaceMember,
                BTreeMap::new(),
                TimingDefaults::default(),
                None,
            )
        })
        .collect();
    let workspace = Workspace::new(&db, packages.clone(), BTreeMap::new());
    let resolutions: Vec<_> = packages
        .iter()
        .map(|package| ridl_sem::resolve_package(&db, workspace, *package, std))
        .collect();
    let mut checked: Vec<_> = packages
        .iter()
        .map(|package| ridl_sem::check_package(&db, workspace, *package, std))
        .collect();
    for package in &checked {
        assert_no_errors(&package.diagnostics);
    }
    amend(&mut checked);
    let mut sources = SourceMap::new();
    let diagnostics =
        ridlc::check_design_lints(&db, &packages, &checked, &resolutions, &mut sources);
    (diagnostics, sources)
}

fn shapes(diagnostics: &[Diagnostic]) -> Vec<&Diagnostic> {
    diagnostics
        .iter()
        .filter(|d| d.code.as_str() == "TYPL-224")
        .collect()
}

#[test]
fn duplicate_struct_is_reported_on_the_later_declaration() {
    let a = "package a\nstruct Point3 { x: float, y: float, z: float }\n";
    let b = "package b\nstruct Vec3 { z: float, x: float, y: float }\n";
    let out = workspace(&[("b", b), ("a", a)]);
    let found = shapes(&out.diagnostics);
    assert_eq!(found.len(), 1, "{:?}", out.diagnostics);
    assert_eq!(
        found[0].message,
        "`b.Vec3` has the same 3 fields as `a.Point3`"
    );
    assert_eq!(found[0].severity, Severity::Info);
    assert_eq!(
        site(&out.sources, found[0].primary),
        ("b/source.ridl".into(), 17..21)
    );
    assert_eq!(found[0].labels.len(), 1);
    assert_eq!(text_at(&out.sources, found[0].labels[0].span), "Point3");
}

#[test]
fn duplicate_enum_is_reported() {
    let source = "package a\nenum Z { Low = 0, High = 1 }\nenum A { High = 5, Low = 7 }\nenum B { Low = 8, High = 9 }\n";
    let out = ridlc::check_source("a.ridl", source);
    assert_no_errors(&out.diagnostics);
    let found = shapes(&out.diagnostics);
    assert_eq!(
        found.iter().map(|d| d.message.as_str()).collect::<Vec<_>>(),
        [
            "`a.A` has the same 2 variants as `a.Z`",
            "`a.B` has the same 2 variants as `a.Z`"
        ]
    );
    for (d, token) in found.iter().zip(["A", "B"]) {
        assert_eq!(text_at(&out.sources, d.primary), token);
        assert_eq!(text_at(&out.sources, d.labels[0].span), "Z");
    }
}

#[test]
fn same_simple_type_name_in_two_packages_is_not_a_duplicate() {
    let a = "package a\nstruct Pose { x: float }\nstruct Entry { pose: Pose, index: integer }\n";
    let b = "package b\nstruct Pose { enabled: boolean }\nstruct Entry { pose: Pose, index: integer }\n";
    let out = workspace(&[("a", a), ("b", b)]);
    assert!(shapes(&out.diagnostics).is_empty(), "{:?}", out.diagnostics);
}

#[test]
fn same_variant_count_with_different_names_is_not_a_duplicate() {
    let out = ridlc::check_source(
        "a.ridl",
        "package a\nenum First { Low = 0, High = 1 }\nenum Second { Cold = 0, Hot = 1 }\n",
    );
    assert_no_errors(&out.diagnostics);
    assert!(shapes(&out.diagnostics).is_empty(), "{:?}", out.diagnostics);
}

#[test]
fn shapes_below_the_threshold_are_not_reported() {
    let out = ridlc::check_source(
        "a.ridl",
        "package a\nstruct First { x: float }\nstruct Second { x: float }\nenum One { Low = 0 }\nenum Two { Low = 1 }\n",
    );
    assert_no_errors(&out.diagnostics);
    assert!(shapes(&out.diagnostics).is_empty(), "{:?}", out.diagnostics);
}

#[test]
fn duplicate_shape_canonicalizes_import_aliases_and_nested_local_types() {
    let a =
        "package a\nstruct Pose { x: float }\nstruct First { poses: [Pose; 2], index: integer }\n";
    let b = "package b\nimport a.Pose as Location\nstruct Second { index: integer, poses: [Location; 2] }\nstruct Optional { index: integer, poses: [Location; 2]? }\nstruct Larger { index: integer, poses: [Location; 3] }\n";
    let out = workspace(&[("b", b), ("a", a)]);
    assert_eq!(
        shapes(&out.diagnostics)
            .iter()
            .map(|d| d.message.as_str())
            .collect::<Vec<_>>(),
        ["`b.Second` has the same 2 fields as `a.First`"]
    );
}

#[test]
fn duplicate_shape_excludes_standard_declarations() {
    let standard =
        "package ridl.std\nstruct First { x: float, y: float }\nenum Mode { Low = 0, High = 1 }\n";
    let user =
        "package a\nstruct Second { x: float, y: float }\nenum Choice { Low = 0, High = 1 }\n";
    let (diagnostics, _) = design_source_set(&[("ridl.std", standard), ("a", user)]);
    assert!(shapes(&diagnostics).is_empty(), "{diagnostics:?}");
}

#[test]
fn duplicate_shape_orders_packages_independently_of_source_set_order() {
    let a = "package a\nstruct Z { x: boolean, y: boolean }\nstruct A { y: boolean, x: boolean }\n";
    let b = "package b\nstruct B { x: boolean, y: boolean }\n";
    let c = "package c\nstruct C { x: boolean, y: boolean }\n";
    for packages in [
        vec![("c", c), ("b", b), ("a", a)],
        vec![("a", a), ("b", b), ("c", c)],
    ] {
        let (diagnostics, sources) = design_source_set(&packages);
        let found = shapes(&diagnostics);
        assert_eq!(
            found.iter().map(|d| d.message.as_str()).collect::<Vec<_>>(),
            [
                "`a.A` has the same 2 fields as `a.Z`",
                "`b.B` has the same 2 fields as `a.Z`",
                "`c.C` has the same 2 fields as `a.Z`",
            ]
        );
        let first = a.find("Z {").unwrap();
        for d in found {
            assert_eq!(
                site(&sources, d.labels[0].span),
                ("a/source.ridl".into(), first..first + 1)
            );
            assert_eq!(d.labels[0].message, "`a.Z` declared here");
        }
    }
}

#[test]
fn duplicate_shape_preserves_field_name_type_pairs() {
    let source = "package a\nstruct First { x: integer, y: boolean }\nstruct Swapped { x: boolean, y: integer }\nstruct Equivalent { y: boolean, x: integer }\n";
    let out = ridlc::check_source("a.ridl", source);
    assert_no_errors(&out.diagnostics);
    assert_eq!(
        shapes(&out.diagnostics)
            .iter()
            .map(|d| d.message.as_str())
            .collect::<Vec<_>>(),
        ["`a.Equivalent` has the same 2 fields as `a.First`"]
    );
}

fn nested_shape_alias_fixture(local: &str, alias: &str, imports: &str) {
    let a = format!(
        "package a\ntype Key: string [1..8]\nstruct Value {{ flag: boolean }}\nstruct First {{ data: {local}, index: integer }}\n"
    );
    let b = format!("package b\n{imports}struct Second {{ index: integer, data: {alias} }}\n");
    let out = workspace(&[("b", &b), ("a", &a)]);
    assert_eq!(
        shapes(&out.diagnostics)
            .iter()
            .map(|d| d.message.as_str())
            .collect::<Vec<_>>(),
        ["`b.Second` has the same 2 fields as `a.First`"]
    );
}

#[test]
fn duplicate_shape_qualifies_tuple_children() {
    nested_shape_alias_fixture(
        "(item: Value)",
        "(item: Payload)",
        "import a.Value as Payload\n",
    );
}

#[test]
fn duplicate_shape_qualifies_map_keys() {
    nested_shape_alias_fixture(
        "[Key: boolean; 2]",
        "[Identifier: boolean; 2]",
        "import a.Key as Identifier\n",
    );
}

#[test]
fn duplicate_shape_qualifies_map_values() {
    nested_shape_alias_fixture(
        "[integer: Value; 2]",
        "[integer: Payload; 2]",
        "import a.Value as Payload\n",
    );
}

#[test]
fn duplicate_shape_qualifies_stream_elements_in_checked_ir() {
    use ridl_ir::v2::{StreamType, decl, field_type, stream_type, struct_member};
    // Streams are interaction-only in source. Exercise the existing FieldType
    // branch using checked IR, retaining source-indexed outer declarations.
    let a =
        "package a\nstruct Value { flag: boolean }\nstruct First { data: Value, index: integer }\n";
    let b =
        "package b\nimport a.Value as Payload\nstruct Second { index: integer, data: Payload }\n";
    let (diagnostics, _) = design_source_set_with(&[("a", a), ("b", b)], |checked| {
        for package in checked {
            for declaration in &mut package.ir.decls {
                if let Some(decl::Kind::StructDef(def)) = &mut declaration.kind {
                    for member in &mut def.members {
                        if let Some(struct_member::Member::Field(field)) = &mut member.member
                            && field.name == "data"
                        {
                            let ty = field.r#type.as_mut().unwrap();
                            let Some(field_type::Kind::Named(name)) = ty.kind.take() else {
                                panic!("named fixture type")
                            };
                            ty.kind = Some(field_type::Kind::Stream(StreamType {
                                element: Some(stream_type::Element::Named(name)),
                            }));
                        }
                    }
                }
            }
        }
    });
    assert_eq!(
        shapes(&diagnostics)
            .iter()
            .map(|d| d.message.as_str())
            .collect::<Vec<_>>(),
        ["`b.Second` has the same 2 fields as `a.First`"]
    );
}

#[test]
fn duplicate_shape_keeps_nominal_identity_for_equal_type_definitions() {
    let a =
        "package a\nstruct Pose { flag: boolean }\nstruct First { pose: Pose, index: integer }\n";
    let b =
        "package b\nstruct Pose { flag: boolean }\nstruct Second { pose: Pose, index: integer }\n";
    let out = workspace(&[("a", a), ("b", b)]);
    assert!(shapes(&out.diagnostics).is_empty(), "{:?}", out.diagnostics);
}

#[test]
fn duplicate_shape_ignores_field_ordinals_docs_and_initial_values() {
    use ridl_ir::v2::{decl, struct_member};
    let source = "package a\nstruct First { x: integer [0..9] = 1, y: boolean = false }\nstruct Second { y: boolean = true, x: integer [0..9] = 2 }\n";
    let mut metadata = Vec::new();
    let (diagnostics, _) = design_source_set_with(&[("a", source)], |checked| {
        for declaration in &mut checked[0].ir.decls {
            if let Some(decl::Kind::StructDef(def)) = &mut declaration.kind {
                for member in &mut def.members {
                    if let Some(struct_member::Member::Field(field)) = &mut member.member
                        && field.name == "x"
                    {
                        // Set distinct documentation explicitly alongside the
                        // source-derived ordinal and initial value differences.
                        field.doc = format!("Documentation for {}", declaration.name);
                        metadata.push((
                            field.ordinal,
                            field.doc.clone(),
                            field.declared_init.clone(),
                        ));
                    }
                }
            }
        }
    });
    assert_eq!(metadata.len(), 2);
    assert_ne!(metadata[0].0, metadata[1].0);
    assert_ne!(metadata[0].1, metadata[1].1);
    assert_ne!(metadata[0].2, metadata[1].2);
    assert_eq!(
        shapes(&diagnostics)
            .iter()
            .map(|d| d.message.as_str())
            .collect::<Vec<_>>(),
        ["`a.Second` has the same 2 fields as `a.First`"]
    );
}

#[test]
fn duplicate_shape_qualifies_inline_scalar_pattern_constants_in_checked_ir() {
    use ridl_ir::v2::{decl, field_type, struct_member};
    // Inline string scalars are forbidden in source fields. Copy valid named
    // string type data into the existing IR variant to exercise its reference.
    let a = "package a\nconst PATTERN = /^x+$/\ntype Text: string [1..8 match PATTERN]\nstruct First { data: Text, index: integer }\n";
    let b = "package b\nimport a.PATTERN as TEXT_PATTERN\ntype Text: string [1..8 match TEXT_PATTERN]\nstruct Second { index: integer, data: Text }\n";
    let (diagnostics, _) = design_source_set_with(&[("a", a), ("b", b)], |checked| {
        for package in checked {
            let mut scalar = package
                .ir
                .decls
                .iter()
                .find_map(|d| match &d.kind {
                    Some(decl::Kind::TypeDef(def)) if d.name == "Text" => Some(def.clone()),
                    _ => None,
                })
                .unwrap();
            assert!(scalar.constraint.as_ref().unwrap().pattern.is_some());
            // Exercise local and qualified forms of the same referenced
            // constant without depending on the checker's retained spelling.
            scalar.constraint.as_mut().unwrap().pattern_const = Some(
                if package.ir.name == "a" {
                    "PATTERN"
                } else {
                    "a.PATTERN"
                }
                .into(),
            );
            for declaration in &mut package.ir.decls {
                if let Some(decl::Kind::StructDef(def)) = &mut declaration.kind {
                    for member in &mut def.members {
                        if let Some(struct_member::Member::Field(field)) = &mut member.member
                            && field.name == "data"
                        {
                            field.r#type.as_mut().unwrap().kind =
                                Some(field_type::Kind::InlineScalar(Box::new(scalar.clone())));
                        }
                    }
                }
            }
        }
    });
    assert_eq!(
        shapes(&diagnostics)
            .iter()
            .map(|d| d.message.as_str())
            .collect::<Vec<_>>(),
        ["`b.Second` has the same 2 fields as `a.First`"]
    );
}

#[test]
fn workspace_package_edges_exclude_external_targets() {
    use ridl_ir::v2::{decl, field_type, struct_member};
    use std::collections::{BTreeMap, BTreeSet};

    let (diagnostics, _) = design_source_set_with(
        &[
            ("a", "package a\ntype A: boolean\n"),
            ("b", "package b\ntype B: boolean\n"),
            ("c", "package c\ntype C: boolean\n"),
            (
                "e",
                "package e\nimport a.A\nimport b.B\nimport c.C\nstruct Bundle { first: A second: B third: C external: boolean }\n",
            ),
        ],
        |checked| {
            // Supply an external qualified reference in checked IR without
            // requiring an unavailable external package during resolution.
            let package = checked.iter_mut().find(|pkg| pkg.ir.name == "e").unwrap();
            let Some(decl::Kind::StructDef(bundle)) = &mut package.ir.decls[0].kind else {
                panic!("struct fixture")
            };
            let field = bundle
                .members
                .iter_mut()
                .find_map(|member| {
                    if let Some(struct_member::Member::Field(field)) = &mut member.member
                        && field.name == "external"
                    {
                        Some(field)
                    } else {
                        None
                    }
                })
                .unwrap();
            field.r#type.as_mut().unwrap().kind =
                Some(field_type::Kind::Named("foreign.deep.Remote".to_string()));

            let complete = ridlc::deps::package_edges(checked, None);
            assert_eq!(
                complete,
                BTreeMap::from([
                    ("a".to_string(), BTreeSet::new()),
                    ("b".to_string(), BTreeSet::new()),
                    ("c".to_string(), BTreeSet::new()),
                    (
                        "e".to_string(),
                        BTreeSet::from([
                            "a".to_string(),
                            "b".to_string(),
                            "c".to_string(),
                            "foreign.deep".to_string(),
                        ])
                    ),
                ])
            );
            assert_eq!(
                ridlc::deps::workspace_package_edges(&complete),
                BTreeMap::from([
                    ("a".to_string(), BTreeSet::new()),
                    ("b".to_string(), BTreeSet::new()),
                    ("c".to_string(), BTreeSet::new()),
                    (
                        "e".to_string(),
                        BTreeSet::from(["a".to_string(), "b".to_string(), "c".to_string(),])
                    ),
                ])
            );
        },
    );
    assert_no_errors(&diagnostics);
}

#[test]
fn inconsistent_unit_excludes_resolved_standard_types_at_user_sites() {
    let source = "package a\ntype Seconds: s [0.0..100.0 step 0.5]\nstruct User { wait: Seconds }\nstruct Implicit { wait: Duration }\nstruct Explicit { wait: ridl.std.Duration? }\ninterface Timers { command schedule(wait: ridl.std.Duration) @[..1s] event wait: ridl.std.Duration @[100ms..1s] }\n";
    let out = workspace(&[("a", source)]);
    assert!(units(&out.diagnostics).is_empty(), "{:?}", out.diagnostics);
}

#[test]
fn inconsistent_unit_excludes_standard_import_aliases_without_excluding_user_names() {
    let source = "package a\nimport ridl.std.Duration as StandardTime\ntype Duration: s [0.0..100.0 step 0.5]\ntype Tick: ms [0.0..100.0 step 0.5]\nstruct First { wait: Duration }\nstruct Second { wait: Duration? }\nstruct Third { wait: Tick }\nstruct Standard { wait: StandardTime? }\ninterface Timers { command schedule(wait: StandardTime) @[..1s] fixed wait: StandardTime }\n";
    // A controlled standard package makes its import available to the resolver;
    // disk workspace loading exposes standard names implicitly instead.
    let (diagnostics, sources) = design_source_set(&[
        (
            "ridl.std",
            "package ridl.std\ntype Duration: ms [0.0..100.0 step 0.5]\n",
        ),
        ("a", source),
    ]);
    let found = units(&diagnostics);
    assert_eq!(found.len(), 1, "{diagnostics:?}");
    assert_eq!(
        found[0].message,
        "`wait` uses `ms` here; elsewhere `wait` uses `s`"
    );
    let start = source.find("wait: Tick").unwrap();
    assert_eq!(
        site(&sources, found[0].primary),
        ("a/source.ridl".into(), start..start + 4)
    );
}

#[test]
fn package_edges_through_the_real_compiler_count_only_system_requires_and_member_edges() {
    let output = workspace_files(&[
        (
            "a",
            vec![
                (
                    "source.ridl",
                    "package a\ninterface I {}\nservice a.feed: I\n",
                ),
                (
                    "component.rsdl",
                    "package a\ncomponent Producer { offers a.feed }\n",
                ),
            ],
        ),
        (
            "b",
            vec![
                (
                    "source.ridl",
                    "package b\ninterface I {}\nservice b.feed: I\n",
                ),
                (
                    "component.rsdl",
                    "package b\ncomponent Producer { offers b.feed }\n",
                ),
            ],
        ),
        (
            "c",
            vec![
                (
                    "source.ridl",
                    "package c\ninterface I {}\nservice c.feed: I\n",
                ),
                (
                    "component.rsdl",
                    "package c\ncomponent Producer { offers c.feed }\n",
                ),
            ],
        ),
        (
            "d",
            vec![
                (
                    "source.ridl",
                    "package d\ninterface I {}\nservice d.feed: I\n",
                ),
                (
                    "component.rsdl",
                    "package d\ncomponent Producer { offers d.feed }\n",
                ),
            ],
        ),
        (
            "e",
            vec![(
                "component.rsdl",
                "package e\ncomponent Consumer [ external ] { requires a.I requires b.I requires c.I requires d.I }\n",
            )],
        ),
        (
            "top",
            vec![(
                "system.rsdl",
                "package top\nsystem Example { a.Producer b.Producer c.Producer d.Producer e.Consumer }\n",
            )],
        ),
    ]);
    assert!(output.system.is_some());
    let without_system = ridlc::deps::package_edges(&output.checked, None);
    assert!(
        without_system.values().all(|targets| targets.is_empty()),
        "{without_system:?}"
    );
    let complete = ridlc::deps::package_edges(&output.checked, output.system.as_ref());
    let edges = ridlc::deps::workspace_package_edges(&complete);
    let targets = |package: &str| {
        edges[package]
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
    };
    assert_eq!(targets("e"), ["a", "b", "c", "d"], "{edges:?}");
    assert_eq!(targets("top"), ["a", "b", "c", "d", "e"], "{edges:?}");
    for package in ["a", "b", "c", "d"] {
        assert!(edges[package].is_empty(), "{edges:?}");
    }
}
