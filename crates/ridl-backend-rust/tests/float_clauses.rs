//! Compile and run float clauses against an independent exact-rational oracle.

#[path = "support/rustc.rs"]
mod rustc;

use num_rational::BigRational;

#[test]
fn inexact_float_equality_skips_the_interface_with_a_refusal_note() {
    let source = r#"
package clause_test
type Rate: float
interface Bounds {
  command set(rate: Rate) @[..1s] [require rate == 0.1]
}
"#;
    let compiled = ridlc::compile("inexact_float_equality.ridl", source);
    let errors: Vec<_> = compiled
        .diagnostics
        .iter()
        .filter(|d| d.severity == ridl_core::diag::Severity::Error)
        .collect();
    assert!(errors.is_empty(), "{errors:?}");
    let generated = ridl_backend_rust::generate_pipeline(
        &compiled.package,
        &[ridl_backend_rust::WireEncoding::FlatBuffers],
        false,
        &[],
    )
    .expect("the pipeline skips the interface rather than refusing the package")
    .rust_source;
    assert!(
        generated.contains("const __RIDL_NO_FACE_Bounds: () = ();"),
        "the refused interface must leave a note constant:\n{generated}"
    );
    assert!(!generated.contains("pub mod bounds {"), "{generated}");
    let note = generated
        .split("const __RIDL_NO_FACE_Bounds")
        .next()
        .expect("the note precedes its constant")
        .rsplit_once("Interface `Bounds`")
        .expect("the note has an interface headline")
        .1;
    for part in [
        "carries no generated interaction face",
        "cannot translate contract clause `rate == 0.1`",
        "the literal has no exact binary64 value, so `==` and `!=` cannot be checked exactly",
        "`==` and `!=` require an exactly representable binary64 literal",
        "every float comparison requires a finite binary64 bound",
    ] {
        assert!(note.contains(part), "missing `{part}` in note:\n{note}");
    }
}

#[test]
fn generated_float_clauses_match_exact_rational_comparisons() {
    let cases = [
        (
            "Positive",
            "0.1".to_string(),
            BigRational::new(1.into(), 10.into()),
        ),
        (
            "Negative",
            "-0.1".to_string(),
            BigRational::new((-1).into(), 10.into()),
        ),
        (
            "Tiny",
            format!("0.{}1", "0".repeat(323)),
            BigRational::new(1.into(), num_bigint::BigInt::from(10).pow(324)),
        ),
        (
            "NegativeTiny",
            format!("-0.{}1", "0".repeat(323)),
            BigRational::new((-1).into(), num_bigint::BigInt::from(10).pow(324)),
        ),
    ];
    let mut source = String::from("package clause_test\ntype Rate: float\ninterface Bounds {\n");
    for (suffix, literal, _) in &cases {
        for (name, op) in [("le", "<="), ("gt", ">"), ("ge", ">="), ("lt", "<")] {
            source.push_str(&format!(
            "query {name}{suffix}(rate: Rate): Rate @[..1s] [require rate {op} {literal}\nensure result {op} {literal}]\n"
        ));
        }
    }
    source.push_str("}\n");
    let compiled = ridlc::compile("float_clauses.ridl", &source);
    let errors: Vec<_> = compiled
        .diagnostics
        .iter()
        .filter(|d| d.severity == ridl_core::diag::Severity::Error)
        .collect();
    assert!(errors.is_empty(), "{errors:?}");
    let generated = ridl_backend_rust::generate_face_with(
        &compiled.package,
        &[ridl_backend_rust::WireEncoding::FlatBuffers],
    )
    .expect("float comparisons generate")
    .rust_source;
    let mut main = String::from("fn main() {\n");
    // Include both neighbours and their neighbours, plus zero and extremes.
    for value in [
        0.0,
        -0.0,
        -0.1,
        f64::MAX,
        -f64::MAX,
        f64::from_bits(1),
        -f64::from_bits(1),
        f64::from_bits(2),
        -f64::from_bits(2),
        f64::from_bits((-0.1f64).to_bits() - 1),
        f64::from_bits((-0.1f64).to_bits() + 1),
        f64::from_bits(0.1f64.to_bits() - 2),
        f64::from_bits(0.1f64.to_bits() - 1),
        0.1f64,
        f64::from_bits(0.1f64.to_bits() + 1),
    ] {
        let exact = BigRational::from_float(value).expect("finite");
        for (suffix, _, literal) in &cases {
            for (prefix, expected) in [
                ("BoundsLe", &exact <= literal),
                ("BoundsGt", &exact > literal),
                ("BoundsGe", &exact >= literal),
                ("BoundsLt", &exact < literal),
            ] {
                let descriptor = format!("{prefix}{suffix}");
                main.push_str(&format!(r#"
                let value = Rate::new({value:?});
                assert_eq!(<{descriptor} as ridl_rt::contract::Query>::require(&value).is_ok(), {expected}, "require {descriptor} at {value:?}");
                assert_eq!(<{descriptor} as ridl_rt::contract::Query>::ensure(&value, &value).is_ok(), {expected}, "ensure {descriptor} at {value:?}");
            "#));
            }
        }
    }
    main.push_str("}\n");
    rustc::run_program(
        "float_clause_predicates",
        &format!("#![allow(dead_code)]\n{generated}\n{main}"),
    );
}

#[test]
fn generated_exact_literal_clauses_match_exact_rational_comparisons() {
    // Source spellings that are exactly representable: a fraction, a trailing
    // zero in the fraction, a negative value and an integer.
    let cases = [
        ("Half", "0.5", BigRational::new(1.into(), 2.into())),
        ("Padded", "1.50", BigRational::new(3.into(), 2.into())),
        ("NegFive", "-5", BigRational::from_integer((-5).into())),
        ("Zero", "0", BigRational::from_integer(0.into())),
    ];
    let ops = [
        ("le", "Le", "<="),
        ("gt", "Gt", ">"),
        ("ge", "Ge", ">="),
        ("lt", "Lt", "<"),
        ("eq", "Eq", "=="),
        ("ne", "Ne", "!="),
    ];
    let mut source = String::from("package clause_test\ntype Rate: float\ninterface Bounds {\n");
    for (suffix, literal, _) in &cases {
        for (name, _, op) in ops {
            source.push_str(&format!(
                "query {name}{suffix}(rate: Rate): Rate @[..1s] [require rate {op} {literal}\nensure result {op} {literal}]\n"
            ));
        }
    }
    source.push_str("}\n");
    let compiled = ridlc::compile("exact_float_clauses.ridl", &source);
    let errors: Vec<_> = compiled
        .diagnostics
        .iter()
        .filter(|d| d.severity == ridl_core::diag::Severity::Error)
        .collect();
    assert!(errors.is_empty(), "{errors:?}");
    let generated = ridl_backend_rust::generate_face_with(
        &compiled.package,
        &[ridl_backend_rust::WireEncoding::FlatBuffers],
    )
    .expect("exact float comparisons generate")
    .rust_source;
    let mut main = String::from("fn main() {\n");
    for value in [
        0.0,
        -0.0,
        0.25,
        -0.25,
        0.5,
        f64::from_bits(0.5f64.to_bits() - 1),
        f64::from_bits(0.5f64.to_bits() + 1),
        1.5,
        -5.0,
        f64::from_bits(1.5f64.to_bits() + 1),
        f64::from_bits((-5.0f64).to_bits() + 1),
        f64::MAX,
        -f64::MAX,
    ] {
        let exact = BigRational::from_float(value).expect("finite");
        for (suffix, _, literal) in &cases {
            for (_, prefix, op) in ops {
                let expected = match op {
                    "<=" => &exact <= literal,
                    ">" => &exact > literal,
                    ">=" => &exact >= literal,
                    "<" => &exact < literal,
                    "==" => &exact == literal,
                    _ => &exact != literal,
                };
                let descriptor = format!("Bounds{prefix}{suffix}");
                main.push_str(&format!(r#"
                let value = Rate::new({value:?});
                assert_eq!(<{descriptor} as ridl_rt::contract::Query>::require(&value).is_ok(), {expected}, "require {descriptor} at {value:?}");
                assert_eq!(<{descriptor} as ridl_rt::contract::Query>::ensure(&value, &value).is_ok(), {expected}, "ensure {descriptor} at {value:?}");
            "#));
            }
        }
    }
    main.push_str("}\n");
    rustc::run_program(
        "exact_float_clause_predicates",
        &format!("#![allow(dead_code)]\n{generated}\n{main}"),
    );
}
