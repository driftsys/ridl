//! The doc-content test of the Rust backend (the generated-code documentation
//! design, test 4): no doc comment in the generated output contains an issue
//! reference or a URL.
//!
//! An issue reference is `#` followed by a digit, or `driftsys/ridl`. A URL is
//! anything with `://`. Generated docs state the contract for the user of the
//! generated code; an explanation for a maintainer of the emitter belongs in a
//! `//` comment beside the code that emits it.
//!
//! The outputs checked are the checked-in face, `generate` and
//! `generate_pipeline` over every fixture in `tests/fixtures/`, and the skip
//! notes `generate_pipeline` leaves for an interface it refuses for a
//! contract clause and for a call shape.

#[path = "support/ir.rs"]
mod ir;
#[path = "support/podbuf.rs"]
mod podbuf;
#[path = "support/proto3.rs"]
mod proto3;
#[path = "support/rustc.rs"]
mod rustc;

use proc_macro2::{TokenStream, TokenTree};

/// Every doc string in `source`: each `///`, `//!`, `/** */` and `/*! */`
/// comment and each `#[doc = "..."]` attribute, which the tokenizer reads as
/// the same `doc` attribute.
fn doc_strings(source: &str) -> Vec<String> {
    let tokens: TokenStream = source.parse().expect("the generated source tokenizes");
    let mut docs = Vec::new();
    collect(tokens, &mut docs);
    docs
}

fn collect(tokens: TokenStream, docs: &mut Vec<String>) {
    for tree in tokens {
        if let TokenTree::Group(group) = tree {
            if let Some(text) = doc_value(&group.stream()) {
                docs.push(text);
            }
            collect(group.stream(), docs);
        }
    }
}

/// The text of `doc = "..."`, the inside of a doc attribute's brackets.
fn doc_value(inner: &TokenStream) -> Option<String> {
    let trees: Vec<TokenTree> = inner.clone().into_iter().collect();
    match trees.as_slice() {
        [
            TokenTree::Ident(name),
            TokenTree::Punct(eq),
            TokenTree::Literal(literal),
        ] if name == "doc" && eq.as_char() == '=' => {
            let lit: syn::LitStr = syn::parse_str(&literal.to_string()).ok()?;
            Some(lit.value())
        }
        _ => None,
    }
}

/// The first forbidden reference in `doc`, if any.
fn forbidden(doc: &str) -> Option<&'static str> {
    let bytes = doc.as_bytes();
    if bytes
        .windows(2)
        .any(|pair| pair[0] == b'#' && pair[1].is_ascii_digit())
    {
        return Some("`#` followed by a digit");
    }
    if doc.contains("driftsys/ridl") {
        return Some("`driftsys/ridl`");
    }
    if doc.contains("://") {
        return Some("a URL (`://`)");
    }
    None
}

/// Fails with every doc string of `source` that holds a forbidden reference.
fn assert_clean(label: &str, source: &str) {
    let docs = doc_strings(source);
    assert!(!docs.is_empty(), "{label}: no doc string was found");
    let offending: Vec<String> = docs
        .iter()
        .filter_map(|doc| forbidden(doc).map(|why| format!("{why}: {doc:?}")))
        .collect();
    assert!(
        offending.is_empty(),
        "{label}: {} generated doc string(s) hold an issue reference or a URL:\n{}",
        offending.len(),
        offending.join("\n"),
    );
}

/// The two members of the proto3 conformance fixture, which reference each
/// other across a package boundary; a single-file compile cannot resolve the
/// import, so [`generate_over_the_conformance_workspace_cites_no_issue`]
/// reads them as the workspace `support/proto3.rs` builds.
const PROTO3_WORKSPACE: [&str; 2] = ["proto3_conformance.ridl", "proto3_conformance_other.ridl"];

/// The two members of the podbuf cross-package fixture: the holder imports
/// the parts, so [`generate_over_the_podbuf_workspace_cites_no_issue`] reads
/// them as the workspace `support/podbuf.rs` builds.
const PODBUF_WORKSPACE: [(&str, &str, &str); 2] = [
    ("parts", "pod.parts", "podbuf_cross_parts.ridl"),
    ("holder", "pod.holder", "podbuf_cross_holder.ridl"),
];

/// The two members of the podbuf nested-union fixture, read the same way.
const PODBUF_NESTED_UNION_WORKSPACE: [(&str, &str, &str); 2] = [
    ("parts", "pod.parts", "podbuf_nested_union_parts.ridl"),
    ("holder", "pod.holder", "podbuf_nested_union_holder.ridl"),
];

/// Whether `name` is a member of a workspace fixture, which a single-file
/// compile cannot read.
fn is_workspace_member(name: &str) -> bool {
    PROTO3_WORKSPACE.contains(&name)
        || PODBUF_WORKSPACE
            .iter()
            .chain(&PODBUF_NESTED_UNION_WORKSPACE)
            .any(|(_, _, file)| *file == name)
}

/// The fixtures the FlatBuffers codec refuses by design, each with the text
/// its refusal must carry: a string with no length bound.
const FLATBUFFERS_REFUSED: [(&str, &str); 1] =
    [("proto3_free.ridl", "` has no finite FlatBuffers bound")];

/// The fixtures the proto3 codec refuses by design, each with the text its
/// refusal must carry — a map keyed by a float and one keyed by bytes, which
/// proto3 does not admit as a key; a declaration with an enum value outside
/// int32, a member the proto backend refuses; and a string with no length
/// bound — so `generate_pipeline` with `proto3` has no output to scan for
/// them, and must refuse for that reason rather than generate. A call whose
/// argument message overflows a `u32` bound (`proto3_unbounded_call.ridl`)
/// is not refused: its interface is skipped with a note, which is scanned
/// like any other output.
const PROTO3_REFUSED: [(&str, &str); 4] = [
    (
        "payload_constraints.ridl",
        "a map key must be an integral or string type",
    ),
    (
        "podbuf_roundtrip.ridl",
        "a map key must be an integral or string type",
    ),
    ("proto3_unbounded.ridl", "` has no proto3 codec: "),
    ("proto3_free.ridl", "` has no proto3 codec: "),
];

/// The fixtures the podbuf codec refuses by design, each with the text its
/// refusal must carry: an array with no count bound, whose largest buffer
/// has no finite podbuf bound.
const PODBUF_REFUSED: [(&str, &str); 2] = [
    ("proto3_free.ridl", "` has no finite podbuf bound"),
    // The podbuf codec keeps every enum strict until driftsys/ridl#1168.
    (
        "open_enums.ridl",
        "` is an open enum, which the podbuf encoding does not carry yet",
    ),
];

/// The fixtures that carry a payload with no finite bound on purpose, which
/// the checker reports as RIDL-157: compiled through
/// `ir::compile_oversized_fixture`, which requires that error and no other.
const OVERSIZED: [&str; 1] = ["proto3_unbounded_call.ridl"];

/// The IR of the fixture `name`, by whichever helper its diagnostics need.
fn compile(name: &str) -> ridl_ir::v2::Package {
    if OVERSIZED.contains(&name) {
        ir::compile_oversized_fixture(name)
    } else {
        ir::compile_fixture(name)
    }
}

fn fixtures() -> Vec<String> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .unwrap_or_else(|error| panic!("read {}: {error}", dir.display()))
        .map(|entry| entry.expect("a fixture entry").file_name())
        .filter_map(|name| name.into_string().ok())
        .filter(|name| name.ends_with(".ridl") && !is_workspace_member(name))
        .collect();
    names.sort();
    assert!(!names.is_empty(), "no fixture in {}", dir.display());
    names
}

/// The detector itself: each forbidden form is found, and plain text is not.
#[test]
fn the_detector_finds_each_forbidden_form() {
    assert!(forbidden("see driftsys/ridl for it").is_some());
    assert!(forbidden("removed by #704").is_some());
    assert!(forbidden("at https://example.org").is_some());
    assert!(forbidden("a `# Rules` heading, decision 8, and C#").is_none());
    let source = "/// first\n#[doc = \"(#713)\"]\nfn f() { /** inner */ struct S; }";
    assert_eq!(
        doc_strings(source),
        vec![
            " first".to_string(),
            "(#713)".to_string(),
            " inner ".to_string()
        ]
    );
}

#[test]
fn the_checked_in_face_cites_no_issue() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/generated/interaction_face.rs");
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    assert_clean("tests/generated/interaction_face.rs", &source);
}

#[test]
fn generate_over_every_fixture_cites_no_issue() {
    for name in fixtures() {
        let package = compile(&name);
        let source = ridl_backend_rust::generate(&package)
            .unwrap_or_else(|error| panic!("generate {name}: {error:?}"))
            .rust_source;
        assert_clean(&format!("generate({name})"), &source);
    }
}

/// `generate_pipeline` over every fixture with FlatBuffers, with proto3 and
/// with podbuf, each for every fixture but the ones that codec refuses by
/// design, which must refuse with the cause [`FLATBUFFERS_REFUSED`],
/// [`PROTO3_REFUSED`] or [`PODBUF_REFUSED`] names; a name listed there that
/// is not a fixture is stale and fails.
#[test]
fn generate_pipeline_over_every_fixture_cites_no_issue() {
    use ridl_backend_rust::WireEncoding;
    let names = fixtures();
    for (refused, _) in FLATBUFFERS_REFUSED
        .iter()
        .chain(&PROTO3_REFUSED)
        .chain(&PODBUF_REFUSED)
    {
        assert!(
            names.iter().any(|name| name == refused),
            "`{refused}` is listed as refused but is not a fixture"
        );
    }
    for name in names {
        let package = compile(&name);
        for (encoding, label, refused) in [
            (WireEncoding::FlatBuffers, "", &FLATBUFFERS_REFUSED[..]),
            (WireEncoding::Proto3, ", proto3", &PROTO3_REFUSED[..]),
            (WireEncoding::Podbuf, ", podbuf", &PODBUF_REFUSED[..]),
        ] {
            let generated = ridl_backend_rust::generate_pipeline(&package, &[encoding], false, &[]);
            if let Some((_, cause)) = refused
                .iter()
                .find(|(refused, _)| *refused == name.as_str())
            {
                let error = generated
                    .err()
                    .unwrap_or_else(|| panic!("{name} is refused with {encoding:?} by design"));
                assert!(
                    error.message.contains(cause),
                    "{name} is refused for the cause {cause:?}, got: {}",
                    error.message
                );
                continue;
            }
            let source = generated
                .unwrap_or_else(|error| {
                    panic!("generate_pipeline {name} with {encoding:?}: {error:?}")
                })
                .rust_source;
            assert_clean(&format!("generate_pipeline({name}{label})"), &source);
        }
    }
}

/// The two-member conformance fixture, read as a workspace: each member
/// generated with the other as its resolution context, with FlatBuffers and
/// with proto3, so the proto3 codec's own doc lines — one `MAX_SIZE` line per
/// `Payload` implementation — are scanned too.
#[test]
fn generate_over_the_conformance_workspace_cites_no_issue() {
    use ridl_backend_rust::WireEncoding;
    let (main, others) = proto3::fixture();
    let mut packages = vec![main];
    packages.extend(others);
    assert_eq!(packages.len(), PROTO3_WORKSPACE.len());
    for (at, package) in packages.iter().enumerate() {
        let others: Vec<&ridl_ir::v2::Package> = packages
            .iter()
            .enumerate()
            .filter(|(other, _)| *other != at)
            .map(|(_, other)| other)
            .collect();
        for encoding in [WireEncoding::FlatBuffers, WireEncoding::Proto3] {
            let source = ridl_backend_rust::generate_pipeline(package, &[encoding], false, &others)
                .unwrap_or_else(|error| {
                    panic!(
                        "generate_pipeline {} with {encoding:?}: {error:?}",
                        package.name
                    )
                })
                .rust_source;
            assert_clean(
                &format!("generate_pipeline({}, {encoding:?})", package.name),
                &source,
            );
        }
    }
}

/// `generate_pipeline` with podbuf over each member of the two podbuf
/// cross-package fixtures, each resolved against the other.
#[test]
fn generate_over_the_podbuf_workspace_cites_no_issue() {
    for workspace in [&PODBUF_WORKSPACE, &PODBUF_NESTED_UNION_WORKSPACE] {
        let packages = podbuf::compile_fixture_workspace(workspace);
        assert_eq!(packages.len(), workspace.len());
        for (at, package) in packages.iter().enumerate() {
            let others: Vec<&ridl_ir::v2::Package> = packages
                .iter()
                .enumerate()
                .filter(|(other, _)| *other != at)
                .map(|(_, other)| other)
                .collect();
            let source = podbuf::generated(package, &others);
            assert_clean(
                &format!("generate_pipeline({}, Podbuf)", package.name),
                &source,
            );
        }
    }
}

/// The skip note of an interface the clause translator refuses, and of one
/// whose call the face cannot carry: each is emitted, so the check is not
/// vacuous, and neither cites an issue.
#[test]
fn the_skip_notes_cite_no_issue() {
    let sources = [
        (
            "clause",
            r#"
package docs.clause

type Level : integer [0..100]

interface Guarded {
  command set(arg: Level) [
    require arg > 10 || arg < 5
  ] @[..50ms]
}
"#,
        ),
        (
            "call shape",
            r#"
package docs.shape

type Level : integer [0..100]

type Fault : integer [0..10]

interface Wide {
  query read(first: Level): Level | Fault @[..50ms]
}
"#,
        ),
    ];
    for (label, text) in sources {
        let output = ridlc::compile(&format!("{label}.ridl"), text);
        let source = ridl_backend_rust::generate_pipeline(
            &output.package,
            &[ridl_backend_rust::WireEncoding::FlatBuffers],
            false,
            &[],
        )
        .expect("generate_pipeline skips rather than refuses")
        .rust_source;
        assert!(
            source.contains("const __RIDL_NO_FACE_"),
            "{label}: the interface must be skipped, or this test proves nothing:\n{source}"
        );
        assert_clean(&format!("the {label} skip note"), &source);
    }
}

/// A link to a member whose Rust name is a raw identifier, and a link to an
/// interaction the async `Client` has no method for, render as code spans:
/// rustdoc cannot resolve either as a path. A signal, a command and a query
/// link to their `Client` method.
#[test]
fn links_rustdoc_cannot_resolve_render_as_code_spans() {
    let package = ir::compile_fixture("doc_links.ridl");
    let source = ridl_backend_rust::generate_pipeline(
        &package,
        &[ridl_backend_rust::WireEncoding::FlatBuffers],
        false,
        &[],
    )
    .expect("the fixture generates")
    .rust_source;
    assert!(source.contains("`veh.links.Reading.loop`"), "{source}");
    assert!(source.contains("`veh.links.Panel.changed`"), "{source}");
    for (written, path) in [
        ("[Panel.level]", "crate::veh::links::panel::Client::level"),
        (
            "[Panel.setLevel]",
            "crate::veh::links::panel::Client::set_level",
        ),
        ("[Panel.read]", "crate::veh::links::panel::Client::read"),
    ] {
        assert!(
            source.contains(&format!("{written}({path})")),
            "{written}\n{source}"
        );
    }
    assert!(!source.contains("r#loop)"), "{source}");
    assert!(!source.contains("Client::changed"), "{source}");
}

/// A package segment that is a Rust keyword puts `r#` in every path of the
/// package, so each link renders as a code span.
#[test]
fn links_into_a_keyword_package_render_as_code_spans() {
    let package = ir::compile_fixture("doc_links_keyword.ridl");
    let source = ridl_backend_rust::generate_pipeline(
        &package,
        &[ridl_backend_rust::WireEncoding::FlatBuffers],
        false,
        &[],
    )
    .expect("the fixture generates")
    .rust_source;
    for target in ["Level", "Reading.level", "Panel.level"] {
        assert!(source.contains(&format!("`veh.move.{target}`")), "{source}");
    }
    assert!(!source.contains("r#move)"), "{source}");
}

/// The pipeline's output for `text`, one source file, with the FlatBuffers
/// codec.
fn pipeline_source(label: &str, text: &str) -> String {
    let output = ridlc::compile(&format!("{label}.ridl"), text);
    ridl_backend_rust::generate_pipeline(
        &output.package,
        &[ridl_backend_rust::WireEncoding::FlatBuffers],
        false,
        &[],
    )
    .expect("generate_pipeline skips rather than refuses")
    .rust_source
}

/// A link to an interface whose face module the output does not hold
/// renders as a code span: `Wide` has no face module, because its only call
/// takes a stream, which the face carries no method for, and `Provisioned`
/// declares only a `fixed` member, which the face carries no method for
/// either. A link to `Panel`, which has a face module, is a link.
#[test]
fn links_to_an_interface_with_no_face_module_render_as_code_spans() {
    let source = pipeline_source(
        "faces",
        r#"
package veh.faces

/// A level, in percent.
type Level: integer [0..100]

/// No face module: a call with a stream parameter has no face method.
interface Wide {
  command set(samples: <Level>) @[..50ms]
}

/// Only a fixed member, so no face module.
interface Provisioned {
  fixed blades: Level
}

/// A face module.
interface Panel {
  /// The current level.
  signal level: Level @10ms
}

/// Links to [Wide], [Provisioned] and [Panel].
struct Links {
  /// The level.
  level: Level
}
"#,
    );
    assert!(
        !source.contains("pub mod wide"),
        "`Wide` must have no face module, or this test proves nothing:\n{source}"
    );
    assert!(
        !source.contains("pub mod provisioned"),
        "`Provisioned` must have no face module, or this test proves nothing:\n{source}"
    );
    assert!(
        source.contains(
            "Links to `veh.faces.Wide`, `veh.faces.Provisioned` and \
             [Panel](crate::veh::faces::panel)."
        ),
        "{source}"
    );
}

/// A target whose name begins with the name of another declaration or
/// interface resolves to its own: `[SpeedLimit]` is not `Speed`, and
/// `[PanelTwo]` is not `Panel`.
#[test]
fn a_link_to_a_name_that_extends_another_resolves_to_its_own_item() {
    let source = pipeline_source(
        "prefix",
        r#"
package veh.prefix

/// A speed.
type Speed: integer [0..250]

/// A speed limit.
type SpeedLimit: integer [0..250]

/// The first panel.
interface Panel {
  /// The current speed.
  signal speed: Speed @10ms
}

/// The second panel.
interface PanelTwo {
  /// The current limit.
  signal limit: SpeedLimit @10ms
}

/// Links to [SpeedLimit] and [PanelTwo].
struct Links {
  /// The speed.
  speed: Speed
}
"#,
    );
    assert!(
        source.contains(
            "Links to [SpeedLimit] and \
             [PanelTwo](crate::veh::prefix::panel_two)."
        ),
        "{source}"
    );
}

#[test]
fn short_links_require_the_rust_name_in_the_docs_module() {
    let package = ir::compile_fixture("doc_short_links.ridl");
    let source = ridl_backend_rust::generate_pipeline(
        &package,
        &[ridl_backend_rust::WireEncoding::FlatBuffers],
        false,
        &[],
    )
    .expect("the fixture generates")
    .rust_source;
    for expected in [
        "Same module [Temperature]; explicit member [Horn.active](crate::veh::docs::horn::Client::active).",
        "Field [Temperature].",
        "Module [Temperature].",
        "Nested [Temperature](crate::veh::docs::Temperature).",
        "Query parameter [Temperature](crate::veh::docs::Temperature).",
        "Command parameter [Temperature](crate::veh::docs::Temperature).",
    ] {
        assert!(source.contains(expected), "missing {expected}:\n{source}");
    }
    for label in ["Query", "Command"] {
        let parameter = format!("{label} parameter [Temperature](crate::veh::docs::Temperature).");
        assert_eq!(
            source.matches(&parameter).count(),
            3,
            "async client, blocking client and Provider must each carry {parameter}:\n{source}"
        );
    }
}

#[test]
fn reference_definitions_cannot_redirect_generated_see_links() {
    let source = pipeline_source(
        "reference-links",
        r#"
package veh.docs
/// A temperature.
type Temperature: integer [0..100]
/// A reading.
///
/// [temperature]: https://example.org/plain
/// [`Temperature`]: https://example.org/code
/// @see Temperature
struct Reading {
  /// A value.
  value: Temperature
}
"#,
    );
    assert!(
        doc_strings(&source)
            .iter()
            .any(|line| line == " - [``Temperature``]"),
        "{source}"
    );
}
