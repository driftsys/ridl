//! Compile and run float clauses against an independent exact-rational oracle.

#[path = "support/rustc.rs"]
mod rustc;

use num_rational::BigRational;

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
