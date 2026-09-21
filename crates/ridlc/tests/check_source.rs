//! `ridlc::check_source`: the single-file front end the MCP tool and
//! `ridl check --format json` share.

use ridl_core::diag::to_json;

#[test]
fn a_bare_package_declaration_checks_clean() {
    let run = ridlc::check_source("only_package.typl", "package p\n");
    assert!(!run.has_error(), "{:?}", run.diagnostics);
}

#[test]
fn a_broken_declaration_reports_an_error_on_line_two() {
    // No trailing newline after `type X:`: the parser reports the missing
    // backing type at the current token, which is end-of-file. A trailing
    // newline moves that position past it, onto a synthetic line three that
    // holds no text — so the fixture omits it to keep the error on line two.
    let run = ridlc::check_source("broken.typl", "package p\ntype X:");
    assert!(run.has_error());

    let json = to_json(&run.diagnostics, &run.sources);
    let error = json
        .iter()
        .find(|diagnostic| diagnostic.severity == "error")
        .expect("an error diagnostic");
    assert!(!error.code.is_empty());
    assert_eq!(error.span.path, "broken.typl");
    assert_eq!(error.span.start.line, 2);
}

#[test]
fn the_profile_follows_the_path_extension() {
    // A `.ridl` file may declare an interface; a `.typl` file may not.
    let source = "package p\ninterface I {}\n";
    assert!(!ridlc::check_source("ok.ridl", source).has_error());
    assert!(ridlc::check_source("wrong.typl", source).has_error());
}

#[test]
fn check_source_and_compile_agree_on_diagnostics() {
    // `compile` always attempts Rust code generation, even when the front end
    // already reported an error, and a codegen failure appends a diagnostic
    // that `check_source` never produces — so this equality holds only for a
    // fixture whose checked IR the backend can still generate from. This is not
    // a general equality between the two functions; a fixture that also breaks
    // the backend would need a different assertion.
    //
    // The fixture is an interface in a `.typl` file: the profile refuses it,
    // and the IR that reaches the backend carries no declaration the backend
    // fails on. It used to be `type X:`, a declaration with no backing, which
    // the backend now refuses in its own right — ADR-0019 decision 8 roots a
    // named scalar in a box table, so a declaration with no backing has no
    // FlatBuffers bound and the codec says so.
    let source = "package p\ninterface I {}\n";
    let checked = ridlc::check_source("same.typl", source);
    let compiled = ridlc::compile("same.typl", source);
    assert_eq!(
        to_json(&checked.diagnostics, &checked.sources),
        to_json(&compiled.diagnostics, &compiled.sources)
    );
}
