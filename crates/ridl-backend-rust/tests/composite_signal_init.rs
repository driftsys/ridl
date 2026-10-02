//! Composite signal init derivability agrees with generated, verified Defaults.
#[path = "support/rustc.rs"]
mod rustc;

fn declarations() -> String {
    let step = format!("1{}.0", "0".repeat(400));
    format!(
        "package app\ntype Bad : float [..-0.1 step {step}]\ntype Good : integer [0..10]\ntype Label : string [1]\nstruct Invalid {{ value : Bad }}\nunion InvalidChoice {{ first : Invalid, later : Good }}\nunion ValidChoice {{ first : Good, later : Invalid }}\nstruct Valid {{ label : Label = \"x\", optional : Bad?, empty : [Bad; 0..2], emptyMap : [integer : Bad; 0..2], one : [integer : Good; 1..2] }}\nstruct Repeated {{ first : Valid, second : Valid }}\n"
    )
}

#[test]
fn derivable_composite_signals_have_valid_generated_defaults() {
    let output = ridlc::compile(
        "composite_signal_init.ridl",
        &format!(
            "{}interface I {{ signal choice : ValidChoice @10ms, signal valid : Valid @10ms, signal repeated : Repeated @10ms }}\n",
            declarations()
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
    for name in ["Invalid", "InvalidChoice"] {
        assert!(
            !source.contains(&format!("impl ::core::default::Default for {name}")),
            "{name} cannot synthesize an invalid first value"
        );
    }
    rustc::run_program(
        "composite_signal_init",
        &format!(
            "#![allow(dead_code)]\n{source}\n{}",
            r#"
use ridl_rt::encoding::FlatBuffers;
use ridl_rt::payload::{Payload, Ref};
fn main() {
    let choice = ValidChoice::default();
    assert!(matches!(choice, ValidChoice::First(value) if value.get() == 0));
    let mut out = vec![0; <ValidChoice as Payload<FlatBuffers>>::MAX_SIZE];
    let bytes = choice.encode(&mut out).unwrap().bytes;
    assert!(matches!(Ref::<'_, ValidChoice, FlatBuffers>::verify(bytes).unwrap().decode(),
        ValidChoice::First(value) if value.get() == 0));
    let valid = Valid::default();
    assert_eq!(valid.label.get(), "x");
    assert!(valid.optional.is_none());
    assert!(valid.empty.is_empty());
    assert!(valid.empty_map.is_empty());
    assert_eq!(valid.one.len(), 1);
    assert_eq!(valid.one[0].0, 0);
    assert_eq!(valid.one[0].1.get(), 0);
    let mut out = vec![0; <Valid as Payload<FlatBuffers>>::MAX_SIZE];
    let bytes = valid.encode(&mut out).unwrap().bytes;
    let back = Ref::<'_, Valid, FlatBuffers>::verify(bytes).unwrap().decode();
    assert_eq!(back.label.get(), "x");
    assert!(back.optional.is_none());
    let repeated = Repeated::default();
    let mut out = vec![0; <Repeated as Payload<FlatBuffers>>::MAX_SIZE];
    let bytes = repeated.encode(&mut out).unwrap().bytes;
    let back = Ref::<'_, Repeated, FlatBuffers>::verify(bytes).unwrap().decode();
    assert_eq!(back.first.label.get(), "x");
    assert_eq!(back.second.label.get(), "x");
}
"#
        ),
    );
}

#[test]
fn nonderivable_composite_signals_are_rejected_before_codegen() {
    for target in ["Invalid", "InvalidChoice", "InvalidKey", "InvalidValue"] {
        let directory = tempfile::tempdir().unwrap();
        let entry = directory.path().join("invalid_composite_signal.ridl");
        let destination = directory.path().join("generated");
        std::fs::write(
            &entry,
            format!(
                "{}struct InvalidKey {{ values : [Label : Good; 1..2] }}\nstruct InvalidValue {{ values : [integer : Bad; 1..2] }}\ninterface I {{ signal value : {target} @10ms }}\n",
                declarations()
            ),
        )
        .unwrap();
        let output = ridlc::run_build(
            &entry,
            &destination,
            &[ridlc::Emit::Rust],
            ridl_core::Frozen::No,
        )
        .unwrap();
        assert!(output.has_error());
        assert!(
            output.diagnostics.iter().any(|diagnostic| {
                diagnostic.code.as_str() == "RIDL-109"
                    && diagnostic.severity == ridl_core::diag::Severity::Error
            }),
            "{target}: {:?}",
            output.diagnostics
        );
        assert!(
            !destination.exists() || std::fs::read_dir(&destination).unwrap().next().is_none(),
            "invalid source cannot emit generated artifacts"
        );
    }
}
