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
            None,
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
    let diagnostics = ridlc::check_design_lints(
        &db,
        &packages,
        &checked,
        &resolutions,
        &checked[1].ir,
        None,
        &mut sources,
    );
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

fn abbreviations(diagnostics: &[Diagnostic]) -> Vec<&Diagnostic> {
    diagnostics
        .iter()
        .filter(|d| d.code.as_str() == "TYPL-223")
        .collect()
}

#[test]
fn abbreviation_is_reported_where_the_short_form_is_used() {
    let source = "package a\nstruct Reading { tempLimit: boolean, temperature: boolean }\n";
    let out = workspace(&[("a", source)]);
    let single = ridlc::check_source("a.ridl", source);
    assert_no_errors(&single.diagnostics);
    for (diagnostics, sources) in [
        (&out.diagnostics, &out.sources),
        (&single.diagnostics, &single.sources),
    ] {
        let found = abbreviations(diagnostics);
        assert_eq!(found.len(), 1, "{diagnostics:?}");
        assert_eq!(
            found[0].message,
            "`temp` in `tempLimit` abbreviates `temperature`, used in `temperature`"
        );
        assert_eq!(found[0].severity, Severity::Info);
        assert_eq!(text_at(sources, found[0].primary), "tempLimit");
        assert_eq!(
            usize::from(found[0].primary.range.start()),
            source.find("tempLimit").unwrap()
        );
    }
}

#[test]
fn abbreviation_needs_three_letters_and_two_more() {
    for (short, long, count) in [
        ("id", "identity", 0),
        ("pos", "post", 0),
        ("pos", "position", 1),
        ("temp", "temperature", 1),
    ] {
        let source = format!("package a\nstruct Reading {{ {short}: boolean, {long}: boolean }}\n");
        let out = workspace(&[("a", &source)]);
        assert_eq!(
            abbreviations(&out.diagnostics).len(),
            count,
            "{short}/{long}: {:?}",
            out.diagnostics
        );
    }
}

#[test]
fn abbreviation_variant_uses_its_own_span() {
    let source = "package a\nenum Mode { Temp = 0 }\nstruct Temperature { value: boolean }\n";
    let out = workspace(&[("a", source)]);
    let found = abbreviations(&out.diagnostics);
    assert_eq!(found.len(), 1, "{:?}", out.diagnostics);
    let start = source.find("Temp =").unwrap();
    assert_eq!(
        site(&out.sources, found[0].primary),
        ("a/source.ridl".to_string(), start..start + 4)
    );
}

#[test]
fn abbreviation_covers_declarations_members_and_parameters_across_packages() {
    let first = "package a\nstruct Temp { value: boolean }\ntype Flag: boolean\ninterface TempInput { command sendTemp(tempValue: Flag) @[..1s] }\nservice a.readings { signal tempReading: Flag @[100ms..1s] }\n";
    let second = "package b\nstruct Temperature { value: boolean }\n";
    let out = workspace(&[("b", second), ("a", first)]);
    let found = abbreviations(&out.diagnostics);
    let names: Vec<_> = found
        .iter()
        .map(|d| text_at(&out.sources, d.primary))
        .collect();
    assert_eq!(
        names,
        ["Temp", "TempInput", "sendTemp", "tempValue", "tempReading"]
    );
    assert!(
        found
            .iter()
            .all(|d| d.message.ends_with("used in `Temperature`"))
    );
}

#[test]
fn abbreviation_reports_each_word_pair_once_per_identifier() {
    let source = "package a\nstruct Reading { tempTempPos: boolean, temperature: boolean, position: boolean, temporary: boolean }\n";
    let out = workspace(&[("a", source)]);
    let messages: Vec<_> = abbreviations(&out.diagnostics)
        .iter()
        .map(|d| d.message.as_str())
        .collect();
    assert_eq!(
        messages,
        [
            "`pos` in `tempTempPos` abbreviates `position`, used in `position`",
            "`temp` in `tempTempPos` abbreviates `temperature`, used in `temperature`",
            "`temp` in `tempTempPos` abbreviates `temporary`, used in `temporary`",
        ]
    );
}

#[test]
fn abbreviation_covers_enumset_bits_and_union_arms_at_their_tokens() {
    let source = "package a\nstruct Temperature { value: boolean }\nenumset Flags { TEMP = 0 }\nunion Choice { temp: Temperature }\n";
    let out = workspace(&[("a", source)]);
    let found = abbreviations(&out.diagnostics);
    assert_eq!(found.len(), 2, "{:?}", out.diagnostics);
    let arm = source.find("temp:").unwrap();
    let bit = source.find("TEMP =").unwrap();
    assert_eq!(
        found
            .iter()
            .map(|d| site(&out.sources, d.primary))
            .collect::<Vec<_>>(),
        [
            ("a/source.ridl".to_string(), arm..arm + 4),
            ("a/source.ridl".to_string(), bit..bit + 4),
        ]
    );
    assert_eq!(
        found.iter().map(|d| d.message.as_str()).collect::<Vec<_>>(),
        [
            "`temp` in `temp` abbreviates `temperature`, used in `Temperature`",
            "`temp` in `TEMP` abbreviates `temperature`, used in `Temperature`",
        ]
    );

    let no_pair = "package a\nstruct Value { value: boolean }\nenumset Flags { TEMP = 0 }\nunion Choice { temp: Value }\n";
    let out = workspace(&[("a", no_pair)]);
    assert!(
        abbreviations(&out.diagnostics).is_empty(),
        "{:?}",
        out.diagnostics
    );
}

fn abbreviation_source_set(packages: &[(&str, &str)]) -> (Vec<Diagnostic>, SourceMap) {
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
                None,
                None,
            )
        })
        .collect();
    let workspace = Workspace::new(&db, packages.clone(), BTreeMap::new());
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
    let std_ir = ridl_sem::check_package(&db, workspace, std, std).ir;
    let mut sources = SourceMap::new();
    let diagnostics = ridlc::check_design_lints(
        &db,
        &packages,
        &checked,
        &resolutions,
        &std_ir,
        None,
        &mut sources,
    );
    (diagnostics, sources)
}

#[test]
fn abbreviation_excludes_standard_short_and_long_words() {
    for (standard, user) in [
        (
            "package ridl.std\nstruct Temperature { value: boolean }\n",
            "package a\nstruct Reading { tempLimit: boolean }\n",
        ),
        (
            "package ridl.std\nstruct Temp { value: boolean }\n",
            "package a\nstruct Reading { temperature: boolean }\n",
        ),
    ] {
        let (diagnostics, _) = abbreviation_source_set(&[("ridl.std", standard), ("a", user)]);
        assert!(abbreviations(&diagnostics).is_empty(), "{diagnostics:?}");
    }
    let source = "package a\nstruct Reading { tempLimit: boolean, temperature: boolean }\n";
    let (diagnostics, sources) = abbreviation_source_set(&[("a", source)]);
    let found = abbreviations(&diagnostics);
    assert_eq!(found.len(), 1, "{diagnostics:?}");
    assert_eq!(text_at(&sources, found[0].primary), "tempLimit");
    assert_eq!(
        found[0].message,
        "`temp` in `tempLimit` abbreviates `temperature`, used in `temperature`"
    );
}

#[test]
fn abbreviation_chooses_the_first_qualified_expansion_independent_of_input_order() {
    let a = "package a\nstruct Z { temperatureEarly: boolean }\n";
    let b = "package b\nstruct A { temperatureLate: boolean }\n";
    let c = "package c\nstruct Reading { tempLimit: boolean }\n";
    for packages in [
        vec![("a", a), ("b", b), ("c", c)],
        vec![("c", c), ("b", b), ("a", a)],
    ] {
        let (diagnostics, sources) = abbreviation_source_set(&packages);
        let found = abbreviations(&diagnostics);
        assert_eq!(found.len(), 1, "{diagnostics:?}");
        assert_eq!(
            found[0].message,
            "`temp` in `tempLimit` abbreviates `temperature`, used in `temperatureEarly`"
        );
        let start = c.find("tempLimit").unwrap();
        assert_eq!(
            site(&sources, found[0].primary),
            ("c/source.ridl".to_string(), start..start + 9)
        );
        assert!(found[0].labels.is_empty(), "{:?}", found[0].labels);
    }
}
