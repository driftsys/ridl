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
        let pkg = v2::Package {
            name: "test".into(),
            decls: vec![v2::Decl {
                name: "Text".into(),
                kind: Some(v2::decl::Kind::TypeDef(v2::TypeDef {
                    backing: Some(v2::Backing {
                        kind: Some(v2::backing::Kind::Primitive(
                            v2::PrimitiveType::String as i32,
                        )),
                    }),
                    constraint: Some(v2::Constraint {
                        pattern: Some(row.literal.clone()),
                        matches: vec![v2::Match {
                            pattern: Some(row.literal.clone()),
                            pattern_const: None,
                        }],
                        ..Default::default()
                    }),
                    ..Default::default()
                })),
                ..Default::default()
            }],
            ..Default::default()
        };
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
