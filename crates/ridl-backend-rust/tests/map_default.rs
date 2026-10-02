//! A map Default must satisfy both its count bound and key uniqueness.
#[path = "support/rustc.rs"]
mod rustc;

#[test]
fn map_defaults_only_construct_zero_or_one_unique_entry() {
    let output = ridlc::compile(
        "map_default.typl",
        "package app\ntype Value : integer [0..3]\ntype Unseeded : string [1]\nstruct Many { values : [integer : Value; 2..3] }\nstruct Zero { values : [integer : Value; 0..3] }\nstruct One { values : [integer : Value; 1..3] }\nstruct BadKey { values : [Unseeded : Value; 1..3] }\nstruct BadValue { values : [integer : Unseeded; 1..3] }\n",
    );
    assert!(
        output
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.severity != ridl_core::diag::Severity::Error),
        "{:?}",
        output.diagnostics
    );
    let generated = ridl_backend_rust::generate(&output.package).unwrap();
    for name in ["Many", "BadKey", "BadValue"] {
        assert!(
            !generated
                .rust_source
                .contains(&format!("impl ::core::default::Default for {name}")),
            "{name} must not synthesize an invalid map init"
        );
    }
    let source = format!(
        "#![allow(dead_code)]\n{}\n{}",
        generated.rust_source,
        r#"
use ridl_rt::encoding::FlatBuffers;
use ridl_rt::payload::Payload;
fn main() {
    let empty = Zero::default();
    assert!(empty.values.is_empty());
    let one = One::default();
    assert_eq!(one.values.len(), 1);
    assert_eq!(one.values[0].0, 0);
    let mut out = vec![0; <Zero as Payload<FlatBuffers>>::MAX_SIZE];
    let bytes = empty.encode(&mut out).unwrap().bytes;
    assert!(<Zero as Payload<FlatBuffers>>::verify(bytes).is_ok());
    let mut out = vec![0; <One as Payload<FlatBuffers>>::MAX_SIZE];
    let bytes = one.encode(&mut out).unwrap().bytes;
    assert!(<One as Payload<FlatBuffers>>::verify(bytes).is_ok());
}
"#
    );
    rustc::run_program("map_default", &source);
}
