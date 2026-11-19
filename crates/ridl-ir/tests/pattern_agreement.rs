//! Shared pattern verdicts for Rust, compatibility searches and TypeScript.

use regex::Regex;
use ridl_ir::{codegen, pattern, v2};
use serde::Deserialize;

#[derive(Deserialize)]
struct Row {
    literal: String,
    normalised: String,
    value: String,
    accepted: bool,
}

#[test]
#[allow(deprecated)]
fn engine_texts_and_lowering_agree_with_the_shared_table() {
    let rows: Vec<Row> = serde_json::from_str(include_str!("pattern_agreement.json")).unwrap();
    for row in rows {
        let n = pattern::normalised_pattern(&row.literal);
        assert_eq!(n, row.normalised, "{}", row.literal);
        let mut db = ridl_core::RidlDatabase::default();
        let std = ridl_core::std_package(&mut db);
        let file = ridl_core::InputFile::new(
            &db,
            "test.typl".into(),
            format!("package test\ntype Text : string [match {}]\n", row.literal),
        );
        let pkg = ridl_core::Package::new(
            &db,
            "test".into(),
            "test".into(),
            vec![file],
            ridl_core::PackageOrigin::WorkspaceMember,
            Default::default(),
            Default::default(),
            None,
        );
        let ws = ridl_core::Workspace::new(&db, vec![pkg], Default::default());
        let checked = ridl_sem::check_package(&db, ws, pkg, std);
        assert!(
            !checked
                .diagnostics
                .iter()
                .any(|d| d.severity == ridl_core::Severity::Error),
            "{}: {:?}",
            row.literal,
            checked.diagnostics
        );
        let pkg = checked.ir;
        let Some(v2::decl::Kind::TypeDef(td)) = &pkg.decls[0].kind else {
            panic!("string type")
        };
        let deprecated_ir = td.constraint.as_ref().unwrap().pattern.as_deref().unwrap();
        let model = codegen::lower(&pkg, &[]);
        let Some(codegen::v1::declaration::Kind::Scalar(sc)) = &model.declarations[0].kind else {
            panic!("string scalar")
        };
        let c = sc.constraint.as_ref().unwrap();
        let lowered = c.matches[0].pattern.as_deref().unwrap();
        for text in [
            pattern::rust_engine_text(lowered),
            pattern::rust_engine_text(&n),
            pattern::search_engine_text(&n),
            c.pattern.clone().unwrap(),
            pattern::pattern_text(deprecated_ir),
        ] {
            assert_eq!(
                Regex::new(&text).unwrap().is_match(&row.value),
                row.accepted,
                "{} with {:?} using {text}",
                row.literal,
                row.value
            );
        }
        assert_eq!(lowered, row.normalised, "{}", row.literal);
    }
}
