//! Executed proof that an enum field's explicit init overrides its type's init.
#[path = "support/rustc.rs"]
mod rustc;

#[test]
fn enum_field_default_uses_the_declared_member_value() {
    let output = ridlc::compile(
        "enum_field_init.typl",
        "package app\ntype Count : integer [0..10]\ntype Label : string [1]\nconst THREE = 3\nenum Mode { OFF = 0, ON = 2, REVERSE = -2 }\nconst ACTIVE = 2\nstruct Settings { literal : Mode = 2.0, constant : Mode = ACTIVE, negative : Mode = -2, derived : Mode, optionalLiteral : Mode? = 2, optionalConstant : Mode? = ACTIVE, optionalCountLiteral : Count? = 3, optionalCountConstant : Count? = THREE, absent : Mode?, optionalLabel : Label? = \"x\" }\nstruct Holder { settings : Settings }\n",
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
    let source = format!(
        "#![allow(dead_code)]\n{}\n{}",
        generated.rust_source,
        r#"
fn main() {
    let value = Settings::default();
    assert!(matches!(value.literal, Mode::On));
    assert!(matches!(value.constant, Mode::On));
    assert!(matches!(value.negative, Mode::Reverse));
    assert!(matches!(value.derived, Mode::Off));
    assert!(matches!(value.optional_literal, Some(Mode::On)));
    assert!(matches!(value.optional_constant, Some(Mode::On)));
    assert_eq!(value.optional_count_literal.unwrap().get(), 3);
    assert_eq!(value.optional_count_constant.unwrap().get(), 3);
    assert!(value.absent.is_none());
    assert_eq!(value.optional_label.unwrap().get(), "x");
    assert!(matches!(Holder::default().settings.optional_constant, Some(Mode::On)));
    assert!(matches!(Holder::default().settings.constant, Mode::On));
}
"#,
    );
    rustc::run_program("enum_field_init", &source);
}

#[test]
fn bytes_defaults_preserve_utf8_init_values() {
    let output = ridlc::compile(
        "bytes_field_init.typl",
        "package app\ntype Declared : bytes [2] = \"é\"\ntype Unseeded : bytes [2]\ntype Empty : bytes [0..2]\nstruct Bytes { required : Unseeded = \"é\", optional : Unseeded? = \"é\", absent : Unseeded?, declared : Declared, empty : Empty }\n",
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
    let source = format!(
        "#![allow(dead_code)]\n{}\n{}",
        generated.rust_source,
        r#"
use ridl_rt::encoding::FlatBuffers;
use ridl_rt::payload::Payload;
fn main() {
    assert_eq!(Declared::default().get(), "é".as_bytes());
    assert!(Empty::default().get().is_empty());
    let value = Bytes::default();
    assert_eq!(value.required.get(), "é".as_bytes());
    assert_eq!(value.optional.as_ref().unwrap().get(), "é".as_bytes());
    assert!(value.absent.is_none());
    assert_eq!(value.declared.get(), "é".as_bytes());
    assert!(value.empty.get().is_empty());
    let mut out = vec![0; <Bytes as Payload<FlatBuffers>>::MAX_SIZE];
    let bytes = value.encode(&mut out).unwrap().bytes;
    assert!(<Bytes as Payload<FlatBuffers>>::verify(bytes).is_ok());
}
"#
    );
    rustc::run_program("bytes_field_init", &source);
}
