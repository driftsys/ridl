//! The declared name of a `service` declaration: one CamelCase identifier.

use ridl_syntax::ast::{AstNode, ServiceDef, SourceFile, is_camel_case_id};
use ridl_syntax::{Profile, parse};

fn first_service(text: &str) -> ServiceDef {
    let parsed = parse(text, Profile::Ridl);
    SourceFile::cast(parsed.syntax())
        .expect("the root is a SourceFile")
        .services()
        .next()
        .expect("one service")
}

#[test]
fn a_one_segment_camel_case_service_name_is_its_declared_name() {
    let service = first_service("package veh.body\ninterface I {}\nservice Doors : I\n");
    assert_eq!(
        service.declared_name().map(|t| t.text().to_string()),
        Some("Doors".into())
    );
}

#[test]
fn a_dotted_or_lowercase_service_name_has_no_declared_name() {
    for text in [
        "service veh.body.doors : I",
        "service doors : I",
        "service Cabin_2 { }",
        "service Doors. : I",
        "service .Doors : I",
    ] {
        let service = first_service(&format!("package p\ninterface I {{}}\n{text}\n"));
        assert!(service.name().is_some(), "`{text}` still parses a name");
        assert_eq!(service.declared_name(), None, "`{text}`");
    }
}

#[test]
fn camel_case_id_is_typl_appendix_e() {
    assert!(is_camel_case_id("Doors") && is_camel_case_id("HVAC") && is_camel_case_id("Doors2"));
    assert!(!is_camel_case_id("doors") && !is_camel_case_id("Cabin_2") && !is_camel_case_id(""));
}
