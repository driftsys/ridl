//! Shared support for the podbuf conformance target.
//!
//! Lives under `tests/support/` (a subdirectory), so Cargo's test-target
//! auto-discovery does not turn it into its own test binary. It is pulled into
//! a test with `#[path = "support/podbuf.rs"] mod podbuf;`.
//!
//! The module holds two kinds of item. The host side builds generated sources
//! and compiles source workspaces. The program side is Rust text that the test
//! appends to a generated codec: the hexadecimal reader, the byte-table
//! checker, and the mutation passes, which have to run inside the compiled
//! program so that one `rustc` run covers every case (a run per case would
//! take hours).

// A `#[path]` module is compiled once per target that names it, and a target
// uses only the items it needs.
#![allow(dead_code)]

use std::path::Path;

use ridl_ir::v2;

/// The generated podbuf codec of `package`, written with `others` as the
/// packages its names resolve in.
pub fn generated(package: &v2::Package, others: &[&v2::Package]) -> String {
    ridl_backend_rust::generate_pipeline(
        package,
        &[ridl_backend_rust::WireEncoding::Podbuf],
        false,
        others,
    )
    .unwrap_or_else(|error| panic!("{} generates: {}", package.name, error.message))
    .rust_source
}

/// Every package of the workspace at `entry`, or the reason it does not
/// compile: an error diagnostic, or a manifest the loader refuses. A corpus
/// workspace that exists to draw diagnostics is not a workspace the layout
/// can be read from.
pub fn compile_workspace(entry: &Path) -> Result<Vec<v2::Package>, String> {
    let mut db = ridl_core::RidlDatabase::default();
    let output = ridlc::compile_workspace(&mut db, entry)
        .map_err(|error| format!("load {}: {error}", entry.display()))?;
    let errors: Vec<_> = output
        .diagnostics
        .iter()
        .filter(|diagnostic| matches!(diagnostic.severity, ridl_core::diag::Severity::Error))
        .map(|diagnostic| diagnostic.code.0.to_string())
        .collect();
    if !errors.is_empty() {
        return Err(format!("{} draws errors: {errors:?}", entry.display()));
    }
    Ok(output
        .checked
        .into_iter()
        .map(|checked| checked.ir)
        .collect())
}

/// Copies the fixtures `members` — `(directory, package name, fixture file)`
/// — into a temporary workspace, as `proto3::fixture` does for the proto3
/// suite, and compiles it. A reference across a package boundary needs a
/// workspace, and a fixture file alone is a single-file synthetic package.
pub fn compile_fixture_workspace(members: &[(&str, &str, &str)]) -> Vec<v2::Package> {
    let dir = tempfile::tempdir().expect("a temp dir is created");
    let names: Vec<&str> = members.iter().map(|(member, _, _)| *member).collect();
    std::fs::write(
        dir.path().join("ridl.toml"),
        format!("[workspace]\nmembers = {names:?}\n"),
    )
    .expect("the workspace manifest is written");
    for (member, package, file) in members {
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
    compile_workspace(dir.path()).unwrap_or_else(|error| panic!("{error}"))
}

/// The bytes a hexadecimal spec names. Whitespace and `|` are separators, so
/// a table row can be written field by field.
pub fn hex_bytes(spec: &str) -> Vec<u8> {
    let digits: Vec<char> = spec
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '|')
        .collect();
    assert!(
        digits.len().is_multiple_of(2),
        "an odd number of hex digits: {spec}"
    );
    digits
        .chunks(2)
        .map(|pair| {
            let text: String = pair.iter().collect();
            u8::from_str_radix(&text, 16).unwrap_or_else(|_| panic!("not hex: {text} in {spec}"))
        })
        .collect()
}

/// A program: `#![allow]` for the dead code and deprecated items the
/// fixtures hold, the generated `modules`, the helpers, and `main`.
pub fn program(modules: &str, helpers: &str, main: &str) -> String {
    format!(
        "#![allow(dead_code, deprecated, unused_imports)]\n{modules}\n{READ_HELPERS}\n{helpers}\n\
         fn main() {{\n{main}\n}}\n"
    )
}

/// The reading helpers every program over a podbuf codec draws on: a
/// hexadecimal reader, and an 8-aligned copy of a buffer (a `Vec<u8>` does
/// not promise the alignment `verify` requires).
pub const READ_HELPERS: &str = r#"
use ridl_rt::encoding::Podbuf;
use ridl_rt::payload::{Payload, Ref, VerifyError};
use ridl_rt::podbuf::Aligned;

/// The bytes a hexadecimal spec names; whitespace and `|` separate.
fn parse(spec: &str) -> Vec<u8> {
    let digits: Vec<char> = spec.chars().filter(|c| !c.is_whitespace() && *c != '|').collect();
    digits
        .chunks(2)
        .map(|pair| u8::from_str_radix(&pair.iter().collect::<String>(), 16).expect("hex"))
        .collect()
}

/// Room for the largest buffer any program here reads.
const CAP: usize = 4096;

/// An 8-aligned copy of `bytes`.
fn aligned(bytes: &[u8]) -> Aligned<[u8; CAP]> {
    let mut out = Aligned([0u8; CAP]);
    out.0[..bytes.len()].copy_from_slice(bytes);
    out
}

/// Binds `$name` to an 8-aligned slice of the bytes `$spec` names.
macro_rules! aligned_bytes {
    ($name:ident, $spec:expr) => {
        let __raw = parse($spec);
        let __buf = aligned(&__raw);
        let $name = &__buf.0[..__raw.len()];
    };
}

/// The bytes `value` encodes to.
fn encode<T: Payload<Podbuf>>(value: &T) -> Vec<u8> {
    let mut buf = <T as Payload<Podbuf>>::BUFFER;
    Ref::<T, Podbuf>::encode(value, buf.as_mut()).expect("encode").bytes().to_vec()
}

/// `OK`, or the text of the refusal.
fn verdict<T: Payload<Podbuf>>(bytes: &[u8]) -> String {
    match Ref::<T, Podbuf>::verify(bytes) {
        Ok(_) => "OK".to_string(),
        Err(VerifyError::Structure(malformed)) => format!("{malformed:?}"),
        Err(VerifyError::Contract(_)) => "Contract".to_string(),
        Err(other) => format!("{other:?}"),
    }
}
"#;

/// The program side of the byte table and the two mutation passes. It needs
/// [`READ_HELPERS`] and value builders from the caller, and exposes `Row`,
/// `row`, `phase` and the three passes.
///
/// `touch::<T>` is the one operation every pass applies to a buffer: verify
/// it as a `T`, and when it is accepted rebuild the value through the view
/// accessors (the caller's `build` calls every accessor, descends into nested
/// views and union arms, drains every iterator and reads every string and
/// byte slice to its end), require it to equal the value `decode` builds
/// (the decoder does not call the view accessors, so this is what exercises
/// them), write the value again, and check that the new bytes verify and
/// decode to the same value. A panic anywhere in it is a defect: a verified
/// buffer must read in bounds.
pub const HARNESS: &str = r#"
/// A small fixed-seed generator, so the random pass runs the same cases
/// every time. Marsaglia's xorshift64.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }

    fn byte(&mut self) -> u8 {
        (self.next() >> 24) as u8
    }
}

type Touch = Box<dyn Fn(&[u8]) -> bool>;

/// Rebuilds a value from the accessors of its view.
type Build<T> = for<'a> fn(&<T as Payload<Podbuf>>::View<'a>) -> T;

/// Verifies `bytes` as a `T` from an aligned copy. Returns whether it was
/// accepted; when it was, reads the whole value through the view and through
/// `decode`, requires the two to agree, and writes it again.
fn touch<T>(bytes: &[u8], build: Build<T>) -> bool
where
    T: Payload<Podbuf> + PartialEq + core::fmt::Debug,
{
    let copy = aligned(bytes);
    let Ok(proof) = Ref::<T, Podbuf>::verify(&copy.0[..bytes.len()]) else {
        return false;
    };
    let rebuilt = build(proof.view());
    let value = proof.decode();
    assert_eq!(rebuilt, value, "the view accessors and decode disagree");
    let mut out = <T as Payload<Podbuf>>::BUFFER;
    let again = Ref::<T, Podbuf>::encode(&value, out.as_mut())
        .expect("a buffer that verified writes again");
    let reverified = Ref::<T, Podbuf>::verify(again.bytes())
        .expect("the bytes written from a verified value verify");
    assert_eq!(reverified.decode(), value);
    true
}

/// One row of the byte table: its name, the operation over its type, and its
/// bytes.
struct Row {
    name: &'static str,
    touch: Touch,
    bytes: Vec<u8>,
}

/// Checks one row: `value` encodes to the bytes `spec` names, those bytes
/// verify, and decoding them gives `value` back.
fn row<T>(rows: &mut Vec<Row>, name: &'static str, value: T, spec: &str, build: Build<T>)
where
    T: Payload<Podbuf> + PartialEq + core::fmt::Debug + 'static,
{
    let expected = parse(spec);
    assert_eq!(encode(&value), expected, "{name}: the bytes the encoder writes");
    assert!(
        <T as Payload<Podbuf>>::MAX_SIZE >= expected.len(),
        "{name}: the bytes are longer than MAX_SIZE"
    );
    let copy = aligned(&expected);
    let proof = Ref::<T, Podbuf>::verify(&copy.0[..expected.len()])
        .unwrap_or_else(|error| panic!("{name}: the expected bytes do not verify: {error:?}"));
    assert_eq!(build(proof.view()), value, "{name}: the value read through the view");
    assert_eq!(proof.decode(), value, "{name}: the value decoded from the bytes");
    rows.push(Row { name, touch: Box::new(move |bytes| touch::<T>(bytes, build)), bytes: expected });
}

/// Runs `body`, and prints one line: its name and `ok` with the summary it
/// returns, or `FAILED` with the panic message.
fn phase(name: &str, body: impl FnOnce() -> String) {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(body)) {
        Ok(summary) => println!("{name} ok {summary}"),
        Err(payload) => {
            let message = payload
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| payload.downcast_ref::<&str>().map(|text| text.to_string()))
                .unwrap_or_default();
            println!("{name} FAILED {}", message.replace('\n', " | "));
        }
    }
}

/// Every prefix of every row's buffer, and every single byte of it replaced
/// by 0x00, 0xff and its bitwise negation. Every byte of every row's buffer
/// is covered by the preamble, the head or a descriptor, so every strict
/// prefix must be refused.
fn exhaustive(rows: &[Row]) -> String {
    let (mut cases, mut accepted) = (0usize, 0usize);
    for row in rows {
        for len in 0..row.bytes.len() {
            let ok = (row.touch)(&row.bytes[..len]);
            cases += 1;
            accepted += usize::from(ok);
            assert!(!ok, "{}: a prefix of {len} of {} bytes was accepted", row.name, row.bytes.len());
        }
        for at in 0..row.bytes.len() {
            for new in [0x00u8, 0xff, !row.bytes[at]] {
                if new == row.bytes[at] {
                    continue;
                }
                let mut bytes = row.bytes.clone();
                bytes[at] = new;
                cases += 1;
                accepted += usize::from((row.touch)(&bytes));
            }
        }
    }
    format!("cases={cases} accepted={accepted}")
}

/// `CASES` seeded cases. Even cases mutate a row's buffer in one to four
/// places (replace a byte, cut the end, append random bytes) and read it as
/// its own type or, one time in four, as another row's type. Odd cases are
/// random buffers of up to 255 bytes read as a random row's type, three in
/// four with a preamble that names a plausible head size, so the pass reaches
/// the checks behind the preamble.
fn random(rows: &[Row]) -> String {
    const CASES: usize = 10_000;
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    let (mut cases, mut accepted) = (0usize, 0usize);
    for case in 0..CASES {
        let (touch, bytes) = if case % 2 == 0 {
            let source = &rows[rng.below(rows.len())];
            let mut bytes = source.bytes.clone();
            for _ in 0..1 + rng.below(4) {
                match rng.below(4) {
                    0 | 1 if !bytes.is_empty() => {
                        let at = rng.below(bytes.len());
                        bytes[at] = rng.byte();
                    }
                    2 if !bytes.is_empty() => bytes.truncate(rng.below(bytes.len())),
                    3 => {
                        for _ in 0..1 + rng.below(8) {
                            bytes.push(rng.byte());
                        }
                    }
                    _ => {}
                }
            }
            let touch = if rng.below(4) == 0 {
                &rows[rng.below(rows.len())].touch
            } else {
                &source.touch
            };
            (touch, bytes)
        } else {
            let mut bytes: Vec<u8> = (0..rng.below(256)).map(|_| rng.byte()).collect();
            if bytes.len() >= 8 && rng.below(4) != 0 {
                let head_size = 8 * rng.below(8) as u32;
                bytes[0..4].copy_from_slice(&head_size.to_le_bytes());
                bytes[4..8].copy_from_slice(&[0; 4]);
            }
            (&rows[rng.below(rows.len())].touch, bytes)
        };
        cases += 1;
        accepted += usize::from(touch(&bytes));
    }
    assert_eq!(cases, CASES);
    format!("cases={cases} accepted={accepted}")
}
"#;
