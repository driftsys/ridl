//! Support for the proto3 conformance test: the two-package fixture, the
//! `.proto` the schema backend emits for it, the `protox` compile of that
//! `.proto`, and the generated codec built once as a program that answers
//! one line per request on standard input.
//!
//! Pulled into a test with `#[path = "support/proto3.rs"] mod proto3;`,
//! beside `#[path = "support/rustc.rs"] mod rustc;`, whose `ridl_rt_rlib`
//! this module links the program against.

#![allow(dead_code)]

use std::collections::BTreeSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use ridl_ir::v2;

/// The name of the package the oracle is generated from.
pub const MAIN: &str = "pb.conformance";

/// The name of the package the main one references across a boundary.
pub const OTHER: &str = "pb.parts";

/// The fixture's two files, in `tests/fixtures/`, and the member directory
/// each is read from.
const MEMBERS: [(&str, &str, &str); 2] = [
    ("conformance", MAIN, "proto3_conformance.ridl"),
    ("parts", OTHER, "proto3_conformance_other.ridl"),
];

/// Compiles the fixture: the main package, then every other package of the
/// workspace, in this case `pb.parts` alone.
///
/// The two fixture files are flat files under `tests/fixtures/`, as every
/// fixture of this crate is, and a cross-package reference needs a workspace
/// to resolve against, so the two are copied into a temporary two-member
/// workspace and read through `ridlc::compile_workspace`, the path a real
/// build takes. Only an error diagnostic fails the compile: the fixture's
/// bare `integer` and `float` fields draw the width warnings on purpose, and
/// its calls carry no response bound.
pub fn fixture() -> (v2::Package, Vec<v2::Package>) {
    let dir = tempfile::tempdir().expect("a temp dir is created");
    let members: Vec<&str> = MEMBERS.iter().map(|(member, _, _)| *member).collect();
    std::fs::write(
        dir.path().join("ridl.toml"),
        format!("[workspace]\nmembers = {members:?}\n"),
    )
    .expect("the workspace manifest is written");
    for (member, package, file) in MEMBERS {
        let member_dir = dir.path().join(member);
        std::fs::create_dir(&member_dir).expect("the member directory is created");
        std::fs::write(
            member_dir.join("ridl.toml"),
            format!("[package]\nname = \"{package}\"\nversion = \"1.0.0\"\n"),
        )
        .expect("the member manifest is written");
        let source = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(file);
        std::fs::copy(&source, member_dir.join(file))
            .unwrap_or_else(|error| panic!("copy {}: {error}", source.display()));
    }
    let mut packages = compile_workspace(dir.path());
    let at = packages
        .iter()
        .position(|package| package.name == MAIN)
        .expect("the fixture declares the main package");
    let main = packages.remove(at);
    (main, packages)
}

/// Every package of the workspace at `entry`, compiled with no error
/// diagnostic. Remote units are left out: they are not the workspace's own.
pub fn compile_workspace(entry: &Path) -> Vec<v2::Package> {
    let mut db = ridl_core::RidlDatabase::default();
    let output = ridlc::compile_workspace(&mut db, entry)
        .unwrap_or_else(|error| panic!("load {}: {error}", entry.display()));
    let errors: Vec<_> = output
        .diagnostics
        .iter()
        .filter(|diagnostic| matches!(diagnostic.severity, ridl_core::diag::Severity::Error))
        .collect();
    assert!(
        errors.is_empty(),
        "{} must compile with no error, got: {errors:?}",
        entry.display()
    );
    output
        .checked
        .into_iter()
        .map(|checked| checked.ir)
        .collect()
}

/// The `.proto` file of every package in `packages`, as `(file name, text)`,
/// each generated with every other package as its resolution context, plus
/// `context` (the standard package, for a corpus that names it). The file
/// name is the one the schema backend's own `import` lines name:
/// `<package>.proto`.
pub fn emit_protos(packages: &[v2::Package], context: &[&v2::Package]) -> Vec<(String, String)> {
    packages
        .iter()
        .enumerate()
        .map(|(at, package)| {
            let mut others: Vec<&v2::Package> = packages
                .iter()
                .enumerate()
                .filter(|(other, _)| *other != at)
                .map(|(_, other)| other)
                .collect();
            others.extend_from_slice(context);
            let generated = ridl_backend_proto::generate_with(package, &others)
                .unwrap_or_else(|error| panic!("{} emits proto3: {}", package.name, error.message));
            (format!("{}.proto", package.name), generated.proto_source)
        })
        .collect()
}

/// The `.proto` of both fixture packages, from `ridl_backend_proto::generate_with`.
pub fn emitted_proto() -> Vec<(String, String)> {
    let (main, others) = fixture();
    let mut packages = vec![main];
    packages.extend(others);
    emit_protos(&packages, &[])
}

/// Compiles `files` with `protox`, every file as a root, and returns the
/// compiler, which holds the descriptor set and the descriptor pool.
///
/// Configured as `protox::compile` configures its own compiler — source info
/// and imports included — so the descriptor set `prost-build` reads is the
/// one that function returns.
pub fn compile_protos(files: &[(String, String)]) -> protox::Compiler {
    let dir = tempfile::tempdir().expect("a temp dir is created");
    for (name, text) in files {
        std::fs::write(dir.path().join(name), text).expect("the schema is written");
    }
    let names: Vec<&str> = files.iter().map(|(name, _)| name.as_str()).collect();
    let mut compiler = protox::Compiler::new([dir.path()]).expect("the include path is read");
    compiler.include_source_info(true).include_imports(true);
    if let Err(error) = compiler.open_files(names) {
        let sources: String = files
            .iter()
            .map(|(name, text)| format!("--- {name}\n{text}"))
            .collect();
        panic!("the emitted schema is not valid proto3:\n{error}\n\n{sources}");
    }
    compiler
}

/// The descriptor pool of the fixture's emitted `.proto`.
pub fn descriptor_pool() -> protox::prost_reflect::DescriptorPool {
    compile_protos(&emitted_proto()).descriptor_pool()
}

/// One value the program encodes on `encode-named`: the `.proto` message
/// name of its type, the name of the row, and the Rust expression of the
/// value in the generated package's vocabulary (`pb.parts` items by their
/// `crate::pb::parts::` path).
pub struct NamedValue {
    pub message: &'static str,
    pub name: &'static str,
    pub expr: &'static str,
}

/// The generated codec, compiled once as a program with a `main` that reads
/// request lines from standard input and answers one line each.
///
/// A request is `<Message> <op> [<argument>]`, where `<Message>` is the
/// `.proto` message name (`Scalars`, `CountBox`) and `<op>` is one of:
///
/// - `verify <hex>`: `ok`, or the `Debug` text of the `VerifyError`;
/// - `decode-encode <hex>`: the hexadecimal re-encoding of the decoded
///   value, or the `Debug` text of the `VerifyError` when `verify` refuses
///   the bytes;
/// - `encode-default`: the hexadecimal encoding of the type's `Default`
///   value, for a message whose generated type has one
///   ([`Program::messages_with_default`]);
/// - `encode-named <name>`: the hexadecimal encoding of the [`NamedValue`]
///   of that name, written into a zeroed buffer;
/// - `encode-named-dirty <name>`: the same, written into a buffer filled
///   with `0xAB`, so a byte the writer leaves untouched shows;
/// - `max-size`: the type's `MAX_SIZE`, in decimal.
///
/// An empty hexadecimal argument is the empty buffer.
pub struct Program {
    binary: PathBuf,
    /// The key of every message the program dispatches — every message the
    /// codec emits with a `Payload<Proto3>`, in every generated package —
    /// and whether its generated type has a `Default` value. The key is the `.proto` message
    /// name, bare or qualified by its package, as [`Program::build_over`]
    /// was asked.
    messages: Vec<(String, bool)>,
    /// Keeps the binary's directory alive as long as the program.
    _dir: tempfile::TempDir,
}

/// How [`Program::build_over`] keys the messages it dispatches.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    /// The bare `.proto` message name (`Scalars`): the fixture's choice,
    /// whose two packages share no name.
    Bare,
    /// The `.proto` full name (`veh.cabin.Scalars`): a corpus workspace's
    /// choice, whose packages may share a bare name.
    Qualified,
}

impl Program {
    /// Builds the program over the fixture: both packages, generated with
    /// the proto3 encoding and composed as `crate::pb::conformance` and
    /// `crate::pb::parts`, `prelude` (helper items the named values use),
    /// and the harness `main`, keyed by bare name. Every message the codec
    /// emits with a `Payload<Proto3>`, in either package, is dispatched; a
    /// message it does not emit, or a tuple
    /// message encoded only inside its owner, is unknown to the program, so
    /// a request over one fails the run.
    pub fn build(prelude: &str, named: &[NamedValue]) -> Program {
        let (main, others) = fixture();
        let mut packages = vec![main];
        packages.extend(others);
        Program::build_over(&packages, Key::Bare, prelude, named)
    }

    /// Builds the program over `packages`, each generated with every other
    /// one as its resolution context, composed into one module tree. A
    /// corpus that names the standard package passes it among `packages`,
    /// since its generated code names `crate::ridl::std::` items. The
    /// harness imports the first package's items for `prelude` and the
    /// named values. Every message the codec emits with a `Payload<Proto3>`,
    /// in any of `packages`, is dispatched under its `key`.
    pub fn build_over(
        packages: &[v2::Package],
        key: Key,
        prelude: &str,
        named: &[NamedValue],
    ) -> Program {
        let mut bodies: Vec<(String, String)> = Vec::new();
        // Per dispatched message: its key, the generated type's path, and
        // whether that type has a `Default` value.
        let mut dispatch: Vec<(String, String, bool)> = Vec::new();
        for (at, package) in packages.iter().enumerate() {
            let others: Vec<&v2::Package> = packages
                .iter()
                .enumerate()
                .filter(|(other, _)| *other != at)
                .map(|(_, other)| other)
                .collect();
            let generated = ridl_backend_rust::generate_pipeline(
                package,
                &[ridl_backend_rust::WireEncoding::Proto3],
                &others,
            )
            .unwrap_or_else(|error| panic!("{} generates: {}", package.name, error.message))
            .rust_source;
            // A generated type has a `Default` only when every field of it
            // has a derivable init value (TYPL-115), which the emitted text
            // states as an `impl Default`; read there, whitespace aside. A
            // text match is deliberate: if the emitter changes the shape of
            // that impl, no message is found to have a `Default`, and the
            // test that requires `Default` on every message but three fails
            // loudly rather than silently checking fewer messages. The
            // `Payload<Proto3>` implementation is read the same way: a
            // message whose type has none — a tuple reached only from a
            // field — is encoded inside its owner and is not dispatched.
            let dense: String = generated.chars().filter(|c| !c.is_whitespace()).collect();
            let layout = ridl_backend_rust::proto3_layout(package, &others)
                .unwrap_or_else(|error| panic!("{} lays out: {}", package.name, error.message));
            let emitted: BTreeSet<&str> =
                layout.iter().map(|message| message.name.as_str()).collect();
            let path = format!("crate::{}", package.name.replace('.', "::"));
            let model = ridl_ir::codegen::lower(package, &others);
            for (message, ty) in message_types(package, &model) {
                let has_codec = dense.contains(&format!(
                    "impl::ridl_rt::payload::Payload<::ridl_rt::encoding::Proto3>for{ty}{{"
                ));
                if emitted.contains(message.as_str()) && has_codec {
                    let has_default =
                        dense.contains(&format!("impl::core::default::Defaultfor{ty}{{"));
                    let key = match key {
                        Key::Bare => message,
                        Key::Qualified => format!("{}.{message}", package.name),
                    };
                    dispatch.push((key, format!("{path}::{ty}"), has_default));
                }
            }
            bodies.push((package.name.clone(), generated));
        }
        let mut seen = BTreeSet::new();
        for (message, _, _) in &dispatch {
            assert!(
                seen.insert(message.clone()),
                "`{message}` is a message of two packages; key the program by full name"
            );
        }
        let arms: String = dispatch
            .iter()
            .map(|(message, ty, _)| format!("        {message:?} => dispatch::<{ty}>(op, arg),\n"))
            .collect();
        let default_arms: String = dispatch
            .iter()
            .filter(|(_, _, has_default)| *has_default)
            .map(|(message, ty, _)| {
                format!("        {message:?} => hex(&encode(&<{ty} as Default>::default())),\n")
            })
            .collect();
        let named_arms: String = named
            .iter()
            .map(|value| {
                format!(
                    "        ({:?}, {:?}) => Some(hex(&encode_into(fill, &{{ {} }}))),\n",
                    value.message, value.name, value.expr
                )
            })
            .collect();
        let main_path = format!(
            "crate::{}",
            packages
                .first()
                .expect("a package to generate")
                .name
                .replace('.', "::")
        );
        // `deprecated` as well: a corpus declaration marked `@deprecated`
        // is emitted with `#[deprecated]`, and the harness's own dispatch
        // names it under `-D warnings`.
        let mut source = String::from("#![allow(dead_code, deprecated)]\n");
        source.push_str(&compose_modules(&bodies));
        source.push_str(&format!(
            r#"
use std::io::BufRead;
use ridl_rt::encoding::Proto3;
use ridl_rt::payload::{{Payload, Ref}};
// The glob import lets the prelude and the named values spell the package's
// types bare, so a package that declares a `String` (ros2's `stdmsgs`) or a
// `Vec` shadows the standard one: the harness's own items spell `::std`.
#[allow(unused_imports)]
use {main_path}::*;

{prelude}

fn hex(bytes: &[u8]) -> ::std::string::String {{
    bytes.iter().map(|byte| format!("{{byte:02x}}")).collect()
}}

fn unhex(text: &str) -> ::std::vec::Vec<u8> {{
    (0..text.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&text[at..at + 2], 16).expect("a hexadecimal byte"))
        .collect()
}}

/// Encodes into a buffer of `MAX_SIZE` bytes filled with `fill`.
fn encode_into<T: Payload<Proto3>>(fill: u8, value: &T) -> ::std::vec::Vec<u8> {{
    let mut out = vec![fill; <T as Payload<Proto3>>::MAX_SIZE];
    <T as Payload<Proto3>>::encode(value, &mut out).expect("encode").bytes.to_vec()
}}

fn encode<T: Payload<Proto3>>(value: &T) -> ::std::vec::Vec<u8> {{
    encode_into(0, value)
}}

fn dispatch<T: Payload<Proto3>>(op: &str, arg: &str) -> ::std::string::String {{
    match op {{
        "verify" => match <T as Payload<Proto3>>::verify(&unhex(arg)) {{
            Ok(_) => ::std::string::String::from("ok"),
            Err(error) => format!("{{error:?}}"),
        }},
        "decode-encode" => {{
            let bytes = unhex(arg);
            match Ref::<'_, T, Proto3>::verify(&bytes) {{
                Ok(proof) => hex(&encode(&proof.decode())),
                Err(error) => format!("{{error:?}}"),
            }}
        }}
        "max-size" => format!("{{}}", <T as Payload<Proto3>>::MAX_SIZE),
        _ => panic!("unknown op `{{op}}`"),
    }}
}}

fn encode_default(message: &str) -> ::std::string::String {{
    match message {{
{default_arms}        _ => panic!("`{{message}}` has no Default value"),
    }}
}}

fn named(message: &str, name: &str, fill: u8) -> ::std::option::Option<::std::string::String> {{
    // Unused by a program with no named value.
    let _ = fill;
    match (message, name) {{
{named_arms}        _ => None,
    }}
}}

fn main() {{
    let stdin = std::io::stdin();
    for line in stdin.lock().lines() {{
        let line = line.expect("a request line");
        let mut parts = line.splitn(3, ' ');
        let message = parts.next().expect("a message name");
        let op = parts.next().expect("an op");
        let arg = parts.next().unwrap_or("");
        let answer = if op == "encode-named" || op == "encode-named-dirty" {{
            let fill = if op == "encode-named" {{ 0 }} else {{ 0xAB }};
            named(message, arg, fill)
                .unwrap_or_else(|| panic!("no named value `{{arg}}` of `{{message}}`"))
        }} else if op == "encode-default" {{
            encode_default(message)
        }} else {{
            match message {{
{arms}                _ => panic!("the codec emits no `{{message}}`"),
            }}
        }};
        println!("{{answer}}");
    }}
}}
"#
        ));

        let dir = tempfile::tempdir().expect("a temp dir is created");
        let source_path = dir.path().join("pb_conformance.rs");
        let binary = dir.path().join("pb_conformance");
        std::fs::write(&source_path, &source).expect("the program source is written");
        let ridl_rt = super::rustc::ridl_rt_rlib(dir.path());
        let status = Command::new("rustc")
            .args(["--edition", "2024", "--crate-type", "bin", "-D", "warnings"])
            .arg("-o")
            .arg(&binary)
            .arg("--extern")
            .arg(format!("ridl_rt={}", ridl_rt.display()))
            .arg(&source_path)
            .status()
            .expect("rustc must be installed and runnable for this test to be meaningful");
        assert!(
            status.success(),
            "the generated codec must compile as a program, source:\n{source}"
        );
        Program {
            binary,
            messages: dispatch
                .into_iter()
                .map(|(message, _, has_default)| (message, has_default))
                .collect(),
            _dir: dir,
        }
    }

    /// The `.proto` name of every message the program dispatches.
    pub fn messages(&self) -> Vec<&str> {
        self.messages
            .iter()
            .map(|(message, _)| message.as_str())
            .collect()
    }

    /// The `.proto` name of every message whose generated type has a
    /// `Default` value, which `encode-default` encodes.
    pub fn messages_with_default(&self) -> Vec<&str> {
        self.messages
            .iter()
            .filter(|(_, has_default)| *has_default)
            .map(|(message, _)| message.as_str())
            .collect()
    }

    /// Feeds `lines` to the program and returns one answer per line.
    pub fn run(&self, lines: &[String]) -> Vec<String> {
        match self.try_run(lines) {
            Ok(answers) => answers,
            Err(Abort { request, stderr }) => panic!(
                "the generated codec must answer every request; it aborted on `{}`, stderr:\n{stderr}",
                lines
                    .get(request)
                    .map(String::as_str)
                    .unwrap_or("<after the last request>")
            ),
        }
    }

    /// Feeds `lines` to the program and returns one answer per line, or the
    /// [`Abort`] that names the request the program died on.
    ///
    /// The program answers each request on its own line and standard output
    /// is line-buffered, so the answers written before a panic are read
    /// back, and the first request without an answer is the one that
    /// panicked.
    pub fn try_run(&self, lines: &[String]) -> Result<Vec<String>, Abort> {
        let mut child = Command::new(&self.binary)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("the compiled program runs");
        let mut stdin = child
            .stdin
            .take()
            .expect("the program reads standard input");
        // The requests are written from a thread while the answers are read
        // below: a batch of thousands of requests fills the answer pipe
        // before the last request is written, and a writer that waits for
        // its own reads to drain that pipe waits forever.
        let requests: Vec<String> = lines.to_vec();
        let writer = std::thread::spawn(move || {
            for line in &requests {
                // A program that died stops reading; the write then fails
                // with a broken pipe, and the exit status below says why.
                if writeln!(stdin, "{line}").is_err() {
                    break;
                }
            }
        });
        let output = child.wait_with_output().expect("the program exits");
        writer.join().expect("the requests are written");
        let answers: Vec<String> = String::from_utf8(output.stdout)
            .expect("the program writes UTF-8")
            .lines()
            .map(str::to_string)
            .collect();
        if !output.status.success() {
            return Err(Abort {
                request: answers.len(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            });
        }
        assert_eq!(
            answers.len(),
            lines.len(),
            "one answer per request, got {answers:?} for {lines:?}"
        );
        Ok(answers)
    }
}

/// A run the program did not finish: the index of the first request it gave
/// no answer to — the one it panicked on — and what it wrote to standard
/// error.
#[derive(Debug)]
pub struct Abort {
    pub request: usize,
    pub stderr: String,
}

/// Composes one Rust body per dotted package name into one module tree, so
/// that `pb.conformance` and `pb.parts` nest under one `pub mod pb`, and a
/// `crate::pb::parts::` path from one body reaches the other.
pub fn compose_modules(bodies: &[(String, String)]) -> String {
    #[derive(Default)]
    struct Tree {
        children: std::collections::BTreeMap<String, Tree>,
        body: Option<String>,
    }
    impl Tree {
        fn insert(&mut self, segments: &[&str], body: &str) {
            match segments.split_first() {
                None => self.body = Some(body.to_string()),
                Some((head, rest)) => self
                    .children
                    .entry((*head).to_string())
                    .or_default()
                    .insert(rest, body),
            }
        }
        fn render(&self, out: &mut String) {
            if let Some(body) = &self.body {
                out.push_str(body);
                out.push('\n');
            }
            for (name, child) in &self.children {
                out.push_str(&format!("pub mod {name} {{\n"));
                child.render(out);
                out.push_str("}\n");
            }
        }
    }
    let mut tree = Tree::default();
    for (package, body) in bodies {
        let segments: Vec<&str> = package.split('.').collect();
        tree.insert(&segments, body);
    }
    let mut out = String::new();
    tree.render(&mut out);
    out
}

/// The `.proto` message name and the generated Rust type name of every
/// declaration of `package` that projects a message — a struct or a union
/// over its own name, a named scalar, an enum or an enum set over
/// `<Name>Box` — and of every induced tuple of the lowered `model`, whose
/// two names the lowering spells.
fn message_types(
    package: &v2::Package,
    model: &ridl_ir::codegen::v1::Model,
) -> Vec<(String, String)> {
    let mut types: Vec<(String, String)> = package
        .decls
        .iter()
        .filter_map(|decl| match decl.kind.as_ref() {
            Some(v2::decl::Kind::StructDef(_) | v2::decl::Kind::UnionDef(_)) => {
                Some((decl.name.clone(), decl.name.clone()))
            }
            Some(
                v2::decl::Kind::TypeDef(_)
                | v2::decl::Kind::EnumDef(_)
                | v2::decl::Kind::EnumSetDef(_),
            ) => Some((format!("{}Box", decl.name), decl.name.clone())),
            _ => None,
        })
        .collect();
    types.extend(model.tuples.iter().filter_map(|tuple| {
        let name = tuple.name.as_ref()?;
        (!name.rust.is_empty()).then(|| (name.wire.clone(), name.rust.clone()))
    }));
    types
}
