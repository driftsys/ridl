//! Executed regressions for scalar constraints and verified map uniqueness.
#[path = "support/ir.rs"]
mod ir;
#[path = "support/rustc.rs"]
mod rustc;

fn program(main: &str) -> String {
    let mut package = ir::compile_fixture("payload_constraints.ridl");
    // Integer step is not a source form; the backend still checks this IR shape.
    for decl in &mut package.decls {
        if decl.name == "Extreme"
            && let Some(ridl_ir::v2::decl::Kind::TypeDef(td)) = &mut decl.kind
        {
            td.constraint.as_mut().unwrap().step = Some("3".into());
        }
    }
    for decl in &mut package.decls {
        let (min, max, step) = match decl.name.as_str() {
            "StepOnly" => (None, None, "0.5"),
            "Tiny" => (None, None, "1e-308"),
            "Wide" => (Some("-1e308"), Some("1e308"), "1e308"),
            "Large" => (Some("1000000000000.0"), None, "0.01"),
            "LargeNegative" => (Some("-1e308"), None, "0.1"),
            _ => continue,
        };
        if let Some(ridl_ir::v2::decl::Kind::TypeDef(td)) = &mut decl.kind {
            td.constraint = Some(ridl_ir::v2::Constraint {
                min: min.map(str::to_owned),
                max: max.map(str::to_owned),
                step: Some(step.into()),
                len_min: None,
                len_max: None,
                pattern: None,
                pattern_const: None,
            });
        }
    }
    let source = ridl_backend_rust::generate(&package).unwrap().rust_source;
    format!("#![allow(dead_code)]\n{source}\n{main}")
}

#[test]
fn constructors_reject_off_grid_values_and_constrained_nan() {
    rustc::run_program(
        "constructors",
        &program(
            r#"
fn main() {
    assert!(Ranged::new(f64::NAN).is_err(), "NaN outside a numeric range");
    assert!(Unconstrained::new(f64::NAN).get().is_nan());
    assert_eq!(Unconstrained::new(f64::INFINITY).get(), f64::INFINITY);
    assert_eq!(Unconstrained::new(f64::NEG_INFINITY).get(), f64::NEG_INFINITY);
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(HalfLower::new(value).is_err(), "implicit upper bound is finite");
        assert!(HalfUpper::new(value).is_err(), "implicit lower bound is finite");
    }
    assert!(HalfLower::new(f64::MAX).is_ok());
    assert!(HalfUpper::new(f64::MIN).is_ok());
    assert!(Decimal::new(0.3).is_ok());
    assert_eq!(Decimal::new(0.35).unwrap_err().rule, ridl_rt::payload::Rule::Step);
    assert!(Decimal::new(f64::NAN).is_err());
    assert!(Offset::new(1.0).is_ok(), "step starts at -3");
    assert!(Offset::new(0.0).is_err());
    assert!(StepOnly::new(1.0).is_ok());
    assert!(StepOnly::new(1.25).is_err());
    assert!(StepOnly::new(f64::NAN).is_err());
    assert!(StepOnly::new(f64::INFINITY).is_err());
    assert!(StepOnly::new(f64::NEG_INFINITY).is_err());
    assert!(Tiny::new(1e308).is_ok(), "dense lattice does not overflow quotient");
    for value in [-1e308, 0.0, 1e308] { assert!(Wide::new(value).is_ok()); }
    assert!(Wide::new(0.5e308).is_err());
    assert!(Large::new(1e12).is_ok());
    assert!(Large::new(1e12 + 0.01).is_ok());
    assert!(Large::new(1e12 + 0.005).is_err(), "large origin must not admit half steps");
    assert!(Extreme::new(i64::MIN).is_ok());
    assert!(Extreme::new(i64::MAX).is_ok(), "subtraction must not overflow");
    assert!(Extreme::new(i64::MAX - 1).is_err());
}
"#,
        ),
    );
}

#[test]
fn verify_enforces_anonymous_scalar_constraints() {
    rustc::run_program(
        "inline_constraints",
        &program(
            r#"
use ridl_rt::encoding::FlatBuffers;
use ridl_rt::payload::Payload;
fn verify(value: &Inline) -> Result<(), ridl_rt::payload::VerifyError> {
    let mut buf = vec![0; 4096];
    let bytes = value.encode(&mut buf).unwrap().bytes;
    <Inline as Payload<FlatBuffers>>::verify(bytes).map(|_| ())
}
fn main() {
    let mut value = Inline { text: "abcdefgh".into(), count: 2, ratio: 0.5, data: vec![1,2] };
    assert!(verify(&value).is_ok());
    value.text = "short".into();
    assert!(verify(&value).is_err(), "inline string minimum");
    value.text = "éééééééé".into();
    assert!(verify(&value).is_ok(), "length is Unicode characters");
    value.count = 4;
    assert!(verify(&value).is_err(), "inline integer bound");
    value.count = 2;
    value.ratio = f64::NAN;
    assert!(verify(&value).is_err(), "inline constrained NaN");
    value.ratio = 0.35;
    assert!(verify(&value).is_err(), "inline floating step");
    value.ratio = 0.3;
    assert!(verify(&value).is_ok(), "decimal f32 wire grid point");
    value.data = vec![1];
    assert!(verify(&value).is_err(), "inline byte minimum");
}
"#,
        ),
    );
}

#[test]
fn verify_rejects_unbounded_default_string_keys_and_duplicate_keys() {
    rustc::run_program(
        "map_constraints",
        &program(
            r#"
use ridl_rt::encoding::FlatBuffers;
use ridl_rt::payload::Payload;
fn verify(value: &Keys) -> Result<(), ridl_rt::payload::VerifyError> {
    let mut buf = vec![0; 4096];
    let bytes = value.encode(&mut buf).unwrap().bytes;
    <Keys as Payload<FlatBuffers>>::verify(bytes).map(|_| ())
}
fn main() {
    let mut value = Keys { text: vec![("a".into(), Whole::new(1)), ("b".into(), Whole::new(2))], number: vec![] };
    assert!(verify(&value).is_ok());
    value.text[1].0 = "a".into();
    assert!(verify(&value).is_err(), "duplicate textual key");
    value.text = vec![("x".repeat(257), Whole::new(1))];
    assert!(verify(&value).is_err(), "unnamed key default [0..256]");
    value.text = vec![];
    value.number = vec![(i64::MIN, Whole::new(1)), (i64::MIN, Whole::new(2))];
    assert!(verify(&value).is_err(), "duplicate integer key");
    value.number[1].0 = i64::MAX;
    assert!(verify(&value).is_ok());
}
"#,
        ),
    );
}

#[test]
fn verified_decimal_wire_values_satisfy_the_named_scalar_check() {
    rustc::run_program(
        "wire_steps",
        &program(
            r#"
use ridl_rt::encoding::FlatBuffers;
use ridl_rt::payload::{Payload, Ref, Rule, VerifyError};
fn main() {
    let value = Decimal::new(0.3).unwrap();
    let mut out = vec![0; <Decimal as Payload<FlatBuffers>>::MAX_SIZE];
    let mut bytes = value.encode(&mut out).unwrap().bytes.to_vec();
    let back = Ref::<'_, Decimal, FlatBuffers>::verify(&bytes).unwrap().decode();
    assert!(Decimal::check(&back.get()).is_ok(), "decoded f32 lattice point remains nominally valid");
    assert!(Decimal::new(back.get()).is_ok());
    let root = ridl_rt::flatbuffers::root(&bytes).unwrap();
    let at = ridl_rt::flatbuffers::field(&bytes, root, 0, 4).unwrap().unwrap();
    bytes[at..at+4].copy_from_slice(&0.35f32.to_le_bytes());
    assert!(matches!(<Decimal as Payload<FlatBuffers>>::verify(&bytes),
        Err(VerifyError::Contract(v)) if v.rule == Rule::Step));
}
"#,
        ),
    );
}

#[test]
fn floating_and_byte_key_equality_is_checked_without_decoding() {
    rustc::run_program(
        "key_equality",
        &program(
            r#"
use ridl_rt::encoding::FlatBuffers;
use ridl_rt::payload::{Payload, Rule, VerifyError};
fn floating(value: &FloatingKeys) -> Result<(), VerifyError> {
    let mut out = vec![0; 4096];
    let bytes = value.encode(&mut out).unwrap().bytes;
    <FloatingKeys as Payload<FlatBuffers>>::verify(bytes).map(|_| ())
}
fn bytes(value: &ByteKeys) -> Result<(), VerifyError> {
    let mut out = vec![0; 4096];
    let bytes = value.encode(&mut out).unwrap().bytes;
    <ByteKeys as Payload<FlatBuffers>>::verify(bytes).map(|_| ())
}
fn main() {
    let whole = || Whole::new(1);
    assert!(floating(&FloatingKeys { entries: vec![] }).is_ok());
    assert!(matches!(floating(&FloatingKeys { entries: vec![(0.0, whole()), (-0.0, whole())] }),
        Err(VerifyError::Contract(v)) if v.rule == Rule::Unique));
    assert!(floating(&FloatingKeys { entries: vec![(f64::NAN, whole()), (f64::NAN, whole())] }).is_ok(),
        "NaN keys are not equal by floating-point equality");
    assert!(bytes(&ByteKeys { entries: vec![(vec![1], whole()), (vec![2], whole())] }).is_ok());
    assert!(matches!(bytes(&ByteKeys { entries: vec![(vec![1], whole()), (vec![1], whole())] }),
        Err(VerifyError::Contract(v)) if v.rule == Rule::Unique));
    assert!(bytes(&ByteKeys { entries: vec![(vec![1], whole()), (vec![2;257], whole())] }).is_err(),
        "an invalid trailing key is checked even when distinct");
}
"#,
        ),
    );
}

#[test]
fn anonymous_pattern_validation_obeys_the_feature() {
    let main = r#"
use ridl_rt::encoding::FlatBuffers;
use ridl_rt::payload::{Payload, Rule, VerifyError};
fn verify(text: &str) -> Result<(), VerifyError> {
    let value = Pattern { text: text.into() };
    let mut out = vec![0; <Pattern as Payload<FlatBuffers>>::MAX_SIZE];
    let bytes = value.encode(&mut out).unwrap().bytes;
    <Pattern as Payload<FlatBuffers>>::verify(bytes).map(|_| ())
}
fn main() {
    assert!(verify("ABC").is_ok());
    if cfg!(feature = "validate-pattern") {
        assert!(matches!(verify("XYZ"), Err(VerifyError::Contract(v)) if v.rule == Rule::Pattern));
    } else {
        assert!(verify("XYZ").is_ok());
    }
    assert!(matches!(verify("AB"), Err(VerifyError::Contract(v)) if v.rule == Rule::Length));
}
"#;
    let source = program(main);
    rustc::run_program("inline_pattern_off", &source);
    rustc::run_program_with_pattern("inline_pattern_on", &source, true);
}

#[test]
fn decimal_lattices_survive_large_indices_and_subnormal_steps() {
    rustc::run_program(
        "decimal_compensation",
        &program(
            r#"
fn main() {
    let corrected = Corrected::new(892794.6706715176).expect("rounded exact decimal lattice point");
    assert_eq!(corrected.get(), 892794.6706715176, "input is preserved");
    assert!(Corrected::new(892794.6706715177).is_err(), "neighbor outside the sparse grid");
    assert!(DenseOrigin::new(30859282352002892.0).is_ok(), "step fits a rounding cell");
    assert!(LargeNegative::new(0.0).is_ok(), "phase reduction avoids a huge index");
    for value in [0.0, 0.5, 1.0] { assert!(Underflow::new(value).is_ok()); }
    assert!(Underflow::new(f64::NAN).is_err());
    assert!(Underflow::new(f64::INFINITY).is_err());
    assert!(Subnormal::new(Subnormal::default().get()).is_ok(), "subnormal decimal residual survives");
    assert!(Subnormal::new(9e-324).is_ok());
    assert!(Subnormal::new(27e-324).is_ok());
    assert!(Subnormal::new(4.9406564584124654e-324).is_err(), "first subnormal is not on this grid");
    assert!(OverflowStep::new(0.0).is_ok(), "a huge step retains its zero lattice point");
    assert!(OverflowStep::new(1.0).is_err());
    assert!(OverflowStep::new(f64::NAN).is_err());
    assert!(OverflowStep::check(&OverflowStep::default().get()).is_ok());
    let default = Underflow::default();
    assert!(Underflow::check(&default.get()).is_ok());
}
"#,
        ),
    );
}

#[test]
fn narrow_wire_projection_preserves_step_validation() {
    rustc::run_program(
        "f32_step_parity",
        &program(
            r#"
use ridl_rt::encoding::FlatBuffers;
use ridl_rt::payload::{Payload, Ref};
fn main() {
    let value = Fine::new(0.7500008).expect("decimal grid point");
    assert_eq!(value.get(), 0.7500008, "the constructor preserves its input");
    let mut out = vec![0; <Fine as Payload<FlatBuffers>>::MAX_SIZE];
    let bytes = value.encode(&mut out).unwrap().bytes;
    let decoded = Ref::<'_, Fine, FlatBuffers>::verify(bytes).expect("the encoded value verifies").decode();
    assert!(Fine::check(&decoded.get()).is_ok());
    assert!(Fine::new(0.75000084).is_err(), "a distinct off-grid wire image is rejected");
}
"#,
        ),
    );
}

#[test]
fn foreign_absent_zero_is_valid_for_extreme_zero_origin_steps() {
    rustc::run_program(
        "extreme_absent_zero",
        &program(
            r#"
use ridl_rt::encoding::FlatBuffers;
use ridl_rt::payload::{Payload, Ref};
fn main() {
    // A minimal foreign FlatBuffers root table: its vtable has no fields.
    let bytes = [8u8,0,0,0, 4,0,4,0, 4,0,0,0, 0,0,0,0];
    assert_eq!(Ref::<'_, Underflow, FlatBuffers>::verify(&bytes).unwrap().decode().get(), 0.0);
    assert_eq!(Ref::<'_, OverflowStep, FlatBuffers>::verify(&bytes).unwrap().decode().get(), 0.0);
    assert!(<Underflow as Payload<FlatBuffers>>::verify(&bytes).is_ok());
    let inline = Ref::<'_, ExtremeInline, FlatBuffers>::verify(&bytes).unwrap().decode();
    assert_eq!(inline.fine, 0.0);
    assert_eq!(inline.huge, 0.0);
}
"#,
        ),
    );
}

#[test]
fn inline_float_range_rejects_on_grid_outside_values_and_range_only_nan() {
    rustc::run_program(
        "inline_float_range",
        &program(
            r#"
use ridl_rt::encoding::FlatBuffers;
use ridl_rt::payload::{Payload, Rule, VerifyError};
fn verify(value: &InlineRange) -> Result<(), VerifyError> {
    let mut out = vec![0; <InlineRange as Payload<FlatBuffers>>::MAX_SIZE];
    let bytes = value.encode(&mut out).unwrap().bytes;
    <InlineRange as Payload<FlatBuffers>>::verify(bytes).map(|_| ())
}
fn main() {
    let mut value = InlineRange { grid: 100.0, range_only: 0.5 };
    assert!(verify(&value).is_ok());
    for outside in [9.0, 150.0] {
        value.grid = outside;
        assert!(matches!(verify(&value), Err(VerifyError::Contract(v)) if v.rule == Rule::Range),
            "on-grid finite value {outside} must fail its inline range");
    }
    value.grid = 100.0;
    value.range_only = f64::NAN;
    assert!(matches!(verify(&value), Err(VerifyError::Contract(v)) if v.rule == Rule::Range),
        "range-only NaN rejection must not depend on Step");
}
"#,
        ),
    );
}

#[test]
fn overflowing_step_retains_nonzero_lattice_points_and_wire_parity() {
    let magnitude = format!("1{}.0", "0".repeat(308));
    let step = format!("2{}.0", "0".repeat(308));
    let output = ridlc::compile(
        "overflow_nonzero.typl",
        &format!(
            "package app\ntype Opposite : float [-{magnitude}..{magnitude} step {step}] = -{magnitude}\n"
        ),
    );
    assert!(
        output
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.severity != ridl_core::diag::Severity::Error),
        "{:?}",
        output.diagnostics
    );
    let source = ridl_backend_rust::generate(&output.package)
        .unwrap()
        .rust_source;
    rustc::run_program(
        "overflow_nonzero",
        &format!(
            "#![allow(dead_code)]\n{source}\n{}",
            r#"
use ridl_rt::encoding::FlatBuffers;
use ridl_rt::payload::{Payload, Ref, Rule};
fn main() {
    assert_eq!(Opposite::default().get(), -1e308);
    assert_eq!(Opposite::new(0.0).unwrap_err().rule, Rule::Step);
    for value in [Opposite::default(), Opposite::new(-1e308).unwrap(), Opposite::new(1e308).unwrap()] {
        let input = value.get();
        assert!(Opposite::check(&input).is_ok());
        let mut out = vec![0; <Opposite as Payload<FlatBuffers>>::MAX_SIZE];
        let bytes = value.encode(&mut out).unwrap().bytes;
        let back = Ref::<'_, Opposite, FlatBuffers>::verify(bytes).unwrap().decode();
        assert_eq!(back.get(), input);
        assert!(Opposite::check(&back.get()).is_ok());
        assert!(Opposite::new(back.get()).is_ok());
    }
}
"#
        ),
    );
}

#[test]
fn generated_step_checks_compile_without_std() {
    let output = ridlc::compile(
        "core_step.typl",
        "package app\ntype Speed : float [0.0..100.0 step 0.1]\n",
    );
    let source = ridl_backend_rust::generate(&output.package)
        .unwrap()
        .rust_source;
    let directory = tempfile::tempdir().unwrap();
    let runtime = rustc::ridl_rt_rlib_without_std(directory.path());
    let path = directory.path().join("core_step.rs");
    std::fs::write(&path, format!("#![no_std]\n#![allow(dead_code)]\n{source}")).unwrap();
    let result = std::process::Command::new("rustc")
        .args([
            "--edition",
            "2021",
            "--crate-type",
            "lib",
            "--emit=metadata",
            "-D",
            "warnings",
        ])
        .arg("--extern")
        .arg(format!("ridl_rt={}", runtime.display()))
        .arg("-o")
        .arg(directory.path().join("core_step.rmeta"))
        .arg(&path)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "no_std emitted checks must compile: {}",
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
fn compensated_reconstruction_does_not_overflow_before_decimal_residual() {
    let maximum = (num_bigint::BigInt::from(1u64 << 53) - 1u32) << 971u32;
    let origin = num_bigint::BigInt::from(14u32) * num_bigint::BigInt::from(10u32).pow(306);
    let step = (&maximum - &origin) / 2u32;
    let output = ridlc::compile(
        "finite_endpoint.typl",
        &format!(
            "package app\ntype Endpoint : float [{origin}.0..{maximum}.0 step {step}.0] = {maximum}.0\n"
        ),
    );
    assert!(
        output
            .diagnostics
            .iter()
            .all(|d| d.severity != ridl_core::diag::Severity::Error),
        "{:?}",
        output.diagnostics
    );
    let source = ridl_backend_rust::generate(&output.package)
        .unwrap()
        .rust_source;
    rustc::run_program(
        "finite_endpoint",
        &format!(
            "#![allow(dead_code)]\n{source}\n{}",
            r#"
use ridl_rt::encoding::FlatBuffers;
use ridl_rt::payload::{Payload, Ref, Rule};
fn main() {
    assert_eq!(Endpoint::default().get(), f64::MAX);
    for value in [Endpoint::default(), Endpoint::new(f64::MAX).unwrap(), Endpoint::new(1.4e307).unwrap()] {
        let input = value.get();
        assert!(Endpoint::check(&input).is_ok());
        let mut out = vec![0; <Endpoint as Payload<FlatBuffers>>::MAX_SIZE];
        let bytes = value.encode(&mut out).unwrap().bytes;
        let back = Ref::<'_, Endpoint, FlatBuffers>::verify(bytes).unwrap().decode();
        assert_eq!(back.get(), input);
        assert!(Endpoint::check(&back.get()).is_ok());
    }
    assert_eq!(Endpoint::new(5e307).unwrap_err().rule, Rule::Step);
}
"#
        ),
    );
}
