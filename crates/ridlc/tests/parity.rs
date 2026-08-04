//! Cross-backend parity over the corpus (docs/ROADMAP.md epic E2 exit
//! criterion: IR neutrality demonstrated by two backends).
//!
//! # Why this file exists
//!
//! Three defects in E2 were the same defect: **one backend carried an IR fact
//! the other dropped**, and each was found by a human reading two golden files
//! side by side.
//!
//! - The synthesized transport identity of an inline `T | E` return: the Rust
//!   backend keyed it on its own mangled type name and the TypeScript backend
//!   on an inline shape's empty `Interface.name`, while `ridl diff` keyed it on
//!   the dotted service name — three values for one wire identity (ADR-0008
//!   decision 4).
//! - `internal` on an interface: both backends dropped it on the interaction
//!   layer, publishing a package-private contract shape, a violation of ADR-0008
//!   decision 7 that shipped in two merged PRs (repaired in #160).
//! - Wire identity: the Rust golden named an ordinal in every interaction's doc
//!   comment and recorded every tombstone, and the TypeScript golden did
//!   neither (repaired in #164).
//!
//! After each repair the guards were TypeScript unit tests, Rust unit tests,
//! and two snapshots that **no assertion related to each other**. Nothing would
//! catch the next instance except another human noticing. The one test that did
//! relate the two —
//! `corpus::internal_on_an_interface_is_package_private_in_both_backends` — is
//! the pattern this file generalizes: one assertion, both backends, over the
//! whole corpus instead of one hand-picked interface.
//!
//! # What is asserted, and against what
//!
//! For every interface in every clean corpus entry — a named `interface` and a
//! service's inline shape alike — **each backend is compared against the IR**,
//! never only against the other backend. Parity follows as a consequence. The
//! distinction matters: both backends render the same IR field and would agree
//! with each other even if that field were wrong, so backend-against-backend
//! would be satisfied by two identically wrong renderings. The ground truth is
//! the checked `ridl_ir::v2::Package` — the same value `corpus.rs` snapshots as
//! `ir@<entry>` — read as typed data.
//!
//! Per interface, both backends must carry:
//!
//! 1. **the ordinal of every interaction**, per face, in emission order —
//!    `Decl.ordinal`, the wire identity a tag-based transport derives its
//!    numeric ids from (ridl §11);
//! 2. **every tombstone with its ordinal** and, for the named form, the retired
//!    name — `Reserved.ordinal` and `Reserved.name`, the record that a freed
//!    slot is never reissued;
//! 3. **the transport identity of every fallible interaction** —
//!    `v2::fallible_transport_identity` applied to IR-derived arguments (see
//!    below);
//! 4. **the visibility of all four generated names** — `Interface.visibility`,
//!    the fact the pre-#160 defect dropped. All four, because the contracts
//!    constant was the one the first report of that defect overlooked;
//! 5. **every contract stub's id, kind and `uses_result`** — `Contract`,
//!    the observer-stub table a runtime installs from (E2.5). `uses_result` is
//!    carried rather than inferred, so a backend dropping it is invisible in
//!    `source`;
//! 6. **every resolved timing bound** — `Timing`: mode, both microsecond
//!    bounds, and `default_applied` (ADR-0008 decision 12).
//!
//! The **face rule** is encoded rather than tripped over, because all three
//! parts of it are deliberate, verified decisions: a consumer face carries every
//! interaction; a provider face carries every interaction except `fixed`
//! (ridl §3, §8 — a `fixed` is provisioned externally and initiated by neither
//! side); and a tombstone is recorded on the consumer face only (it generates no
//! member, so it rides on the interface's own doc). A guard that expected
//! symmetry here would report a defect on every corpus entry.
//!
//! ## Transport identity: why the shared derivation is called
//!
//! The expected identity is built by calling `v2::fallible_transport_identity`
//! — the single derivation both backends and `ridl diff` call — with arguments
//! taken from the IR: the interface's **identity** name (its own name, or a
//! service's dotted global name for an inline shape), `Decl.ordinal`, and the
//! `FallibleType` arms. That is deliberate. The defect was never in the format;
//! it was in the first argument, which each consumer derived for itself. This
//! guard checks the arguments. The format itself is pinned separately, by
//! `ridl_ir::v2_round_trip::fallible_transport_identity_follows_the_derivation_rule`.
//!
//! # What is normalised, and why each is a genuine language difference
//!
//! Only differences the two target languages impose are normalised. Anything
//! else that differs is a finding, and this guard reports it.
//!
//! 1. **Generated interface name.** Rust spells a service's inline shape
//!    `ServiceVehHvacCabin`, TypeScript spells it `Service_veh_hvac_cabin`.
//!    Neither is the interface's identity — that stays the dotted
//!    `veh.hvac.cabin` — and each is the target language's identifier form of
//!    the same thing. [`canonical_key`] reduces a generated name to lowercase
//!    ASCII alphanumerics, which maps both spellings, the Rust
//!    `SERVICE_VEH_HVAC_CABIN_TIMING` const and the TypeScript
//!    `service_veh_hvac_cabinTiming` const onto one key. The key set is asserted
//!    to be collision-free, so the normalisation can never merge two interfaces.
//! 2. **Package-private mechanism.** Rust has `pub(crate)`, TypeScript has the
//!    absence of `export`. There is no third option in either language; both are
//!    mapped to [`Visibility::PackagePrivate`].
//! 3. **Timing mode spelling.** Rust emits the enum variant
//!    `TimingMode::StrictPeriodic`, TypeScript the string-literal-union member
//!    `'strict-periodic'`. TypeScript has no enum in the emitted surface.
//! 4. **Absent timing bound.** Rust emits `None`, TypeScript `undefined` — the
//!    two languages' spellings of "this half-open range has no bound here".
//! 5. **Microsecond literal.** Rust emits `Some(10000)` (a `u64`), TypeScript
//!    `10000n` (a `bigint`, the only TypeScript numeric form that holds an exact
//!    microsecond count without rounding). Both are compared as the IR's exact
//!    decimal text.
//! 6. **Contract-kind spelling.** Rust `ContractKind::Require`, TypeScript
//!    `'require'` — the same enum-versus-string-literal difference as (3).
//!
//! Two things are deliberately **not** normalised, and are worth naming so a
//! reader knows what the parity claim does and does not cover.
//!
//! - **Interaction names are compared verbatim.** Rust names its methods
//!   `current_speed` and TypeScript names its members `currentSpeed`, but both
//!   backends record the *source* name — Rust inside its generated doc comment,
//!   TypeScript as the member name itself — so no case folding is needed and
//!   none is done. What that buys is not the same on both sides, and the
//!   difference is worth stating rather than glossing: in TypeScript the
//!   compared name **is** the emitted identifier, so a renamed member fails
//!   here; in Rust the compared name is read out of doc-comment prose, so
//!   renaming a method while leaving its doc comment alone is **green** here.
//!   That case is caught by the Rust backend's own snapshots, not by this file.
//! - **Where the transport identity is repeated is not asserted.** Rust records
//!   it once, in the consumer face's doc comment; TypeScript repeats it on both
//!   faces. The consumer face is where the identity is pinned: it is compared
//!   interaction by interaction against the IR, so a missing, extra, wrong or
//!   misattributed identity fails there. The provider face is checked for
//!   correctness but not for presence — every `(interaction, identity)` pair
//!   recorded there must be one the IR derives, and a face that records none is
//!   accepted. That is what makes Rust's single record and TypeScript's
//!   duplicate record both pass: the repetition is a doc-comment placement
//!   choice, not information one backend carries and the other drops, and
//!   asserting it would freeze a cosmetic decision into a class-level guard.
//!
//! # Reading the generated source
//!
//! Both backends are parsed from their generated text, because that is the
//! artifact a consumer receives. The Rust source is `prettyplease`-formatted, so
//! anything inside a `const` body is read after collapsing whitespace and is
//! independent of where the formatter breaks lines; doc comments are `///` lines
//! by construction and are read line by line. The TypeScript source is emitted
//! directly with one entry per line.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use ridl_core::db::InputFile;
use ridl_core::diag::Severity;
use ridl_core::package::Package;
use ridl_core::{RidlDatabase, load_workspace, parse_file, std_package};
use ridl_ir::v2;
use ridl_sem::{check_package, resolve_package};

// ==========================================================================
// The facts, normalised.
// ==========================================================================

/// A generated declaration's reach. Rust spells the narrow one `pub(crate)` and
/// TypeScript spells it by omitting `export`; the mechanisms differ because the
/// languages differ, the fact does not.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Visibility {
    Public,
    PackagePrivate,
}

impl Visibility {
    /// The visibility an `Interface` carries in the IR. An inline service shape
    /// leaves it UNSPECIFIED — a service is a global published contract and
    /// takes no `internal` modifier (ridl §14.5) — which is public.
    fn of(visibility: i32) -> Self {
        match v2::Visibility::try_from(visibility).unwrap_or(v2::Visibility::Unspecified) {
            v2::Visibility::Internal => Visibility::PackagePrivate,
            _ => Visibility::Public,
        }
    }
}

/// Which side of a binding a generated face serves (ridl §14).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Face {
    Consumer,
    Provider,
}

impl Face {
    fn label(self) -> &'static str {
        match self {
            Face::Consumer => "consumer face",
            Face::Provider => "provider face",
        }
    }
}

/// One interaction's resolved timing, in the IR's own terms: the mode as the
/// canonical string, both bounds as exact decimal microsecond text, and whether
/// the compiler resolved the bound from the configured default.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Timing {
    mode: String,
    min_us: Option<String>,
    max_us: Option<String>,
    default_applied: bool,
}

/// One contract clause, reduced to the parts a backend cannot re-derive: the
/// observer-stub id, the kind, and `uses_result`.
#[derive(Clone, Debug, PartialEq, Eq)]
struct ContractStub {
    id: String,
    kind: String,
    uses_result: bool,
}

/// What the IR fixes about one interface, and what each backend must render.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Expected {
    /// The interface's identity — its own name, or a service's dotted global
    /// name for an inline shape. Used in failure messages and as the first
    /// component of a transport identity.
    identity: String,
    visibility: Visibility,
    /// `(interaction name, ordinal)` in declaration order, for each face.
    consumer: Vec<(String, u32)>,
    provider: Vec<(String, u32)>,
    /// `(ordinal, retired name)` in declaration order; consumer face only.
    tombstones: Vec<(u32, Option<String>)>,
    /// `(interaction name, transport identity)` for every fallible return.
    transport: Vec<(String, String)>,
    timing: Vec<(String, Timing)>,
    contracts: Vec<ContractStub>,
}

/// One generated face, as read back out of a backend's output.
#[derive(Debug)]
struct RenderedFace {
    visibility: Visibility,
    ordinals: Vec<(String, u32)>,
    transport: Vec<(String, String)>,
    tombstones: Vec<(u32, Option<String>)>,
}

/// A whole generated package, as read back out of a backend's output.
#[derive(Debug, Default)]
struct Rendered {
    faces: BTreeMap<(String, Face), RenderedFace>,
    timing: BTreeMap<String, (Visibility, Vec<(String, Timing)>)>,
    contracts: BTreeMap<String, (Visibility, Vec<ContractStub>)>,
}

/// Reduces a generated name to the key both backends' spellings share: ASCII
/// alphanumerics, lowercased. `ServiceVehHvacCabin`, `Service_veh_hvac_cabin`,
/// `SERVICE_VEH_HVAC_CABIN` and `service_veh_hvac_cabin` all become
/// `servicevehhvaccabin`.
///
/// This is the one name normalisation in this file, and it exists because the
/// two languages impose different identifier forms on the same interface. It is
/// never applied to an interaction name, a contract id, or a transport identity
/// — those are compared verbatim.
fn canonical_key(name: &str) -> String {
    name.chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|ch| ch.to_ascii_lowercase())
        .collect()
}

// ==========================================================================
// Ground truth: the checked IR.
// ==========================================================================

/// The canonical spelling of a timing mode. An unresolved mode is rejected by
/// both backends (`GenerateError`), so a clean entry never reaches this arm.
fn timing_mode(mode: i32) -> String {
    match v2::TimingMode::try_from(mode).unwrap_or(v2::TimingMode::Unspecified) {
        v2::TimingMode::StrictPeriodic => "strict-periodic",
        v2::TimingMode::Range => "range",
        v2::TimingMode::Unspecified => "unspecified",
    }
    .to_string()
}

/// The canonical spelling of a contract kind. Like [`timing_mode`], the
/// unresolved case is named rather than folded into a real one: both backends
/// refuse to generate a kindless clause (`GenerateError`) precisely because
/// guessing installs the observer at the wrong moment, so silently reading it
/// as `require` here would be this file making the guess they refuse to.
fn contract_kind(kind: i32) -> String {
    match v2::ContractKind::try_from(kind).unwrap_or(v2::ContractKind::Unspecified) {
        v2::ContractKind::Require => "require",
        v2::ContractKind::Ensure => "ensure",
        v2::ContractKind::Unspecified => "unspecified",
    }
    .to_string()
}

/// Reads one interface out of the IR into the facts both backends must carry.
///
/// `identity` is what the interface is called outside a backend: its own name,
/// or the service's dotted global name for an inline shape. It is the first
/// component of a transport identity and the prefix of an observer-stub id, so
/// it is passed in rather than read from `Interface.name` — which is `""` by
/// construction for an inline shape (ridl §14.5).
fn expected_interface(identity: &str, interface: &v2::Interface, kinds: &mut Kinds) -> Expected {
    if Visibility::of(interface.visibility) == Visibility::PackagePrivate {
        kinds.package_private_interfaces += 1;
    }
    let mut expected = Expected {
        identity: identity.to_string(),
        visibility: Visibility::of(interface.visibility),
        consumer: Vec::new(),
        provider: Vec::new(),
        tombstones: Vec::new(),
        transport: Vec::new(),
        timing: Vec::new(),
        contracts: Vec::new(),
    };

    for decl in &interface.interactions {
        let name = decl.name.clone();
        let ordinal = decl.ordinal;

        match &decl.kind {
            Some(v2::decl::Kind::SignalDef(signal)) => {
                expected.consumer.push((name.clone(), ordinal));
                expected.provider.push((name.clone(), ordinal));
                if let Some(spec) = &signal.timing {
                    expected.timing.push((name, expected_timing(spec)));
                }
            }
            Some(v2::decl::Kind::EventDef(event)) => {
                expected.consumer.push((name.clone(), ordinal));
                expected.provider.push((name.clone(), ordinal));
                if let Some(spec) = &event.timing {
                    expected.timing.push((name, expected_timing(spec)));
                }
            }
            Some(v2::decl::Kind::CommandDef(command)) => {
                expected.consumer.push((name.clone(), ordinal));
                expected.provider.push((name.clone(), ordinal));
                expected.contracts.extend(stubs_of(&command.contracts));
                // Declared RPC bounds ride the same timing table (ADR-0015);
                // absent means undeclared, never defaulted.
                if let Some(spec) = &command.timing {
                    expected.timing.push((name, expected_timing(spec)));
                    kinds.rpc_bound_timings += 1;
                }
            }
            Some(v2::decl::Kind::QueryDef(query)) => {
                expected.consumer.push((name.clone(), ordinal));
                expected.provider.push((name.clone(), ordinal));
                if let Some(v2::return_type::Kind::Fallible(fallible)) =
                    query.return_type.as_ref().and_then(|rt| rt.kind.as_ref())
                {
                    expected.transport.push((
                        name.clone(),
                        v2::fallible_transport_identity(identity, ordinal, fallible),
                    ));
                }
                expected.contracts.extend(stubs_of(&query.contracts));
                if let Some(spec) = &query.timing {
                    expected.timing.push((name, expected_timing(spec)));
                    kinds.rpc_bound_timings += 1;
                }
            }
            // The face rule: a `fixed` is provisioned externally and initiated
            // by neither side (ridl §3, §8), so it appears on the consumer face
            // and nowhere else. A guard expecting it on both would report a
            // defect on every interface that declares one.
            Some(v2::decl::Kind::FixedDef(_)) => {
                expected.consumer.push((name, ordinal));
                kinds.fixed += 1;
            }
            // A tombstone generates no member, so it is recorded on the
            // interface's own doc — on the consumer face only, in both backends.
            Some(v2::decl::Kind::ReservedSlot(slot)) => {
                assert_eq!(
                    slot.ordinal, ordinal,
                    "{identity}: the tombstone's `Reserved.ordinal` and its `Decl.ordinal` \
                     disagree. Both backends render `Reserved.ordinal` and `ridl diff` walks \
                     `Decl.ordinal`, so a divergence here is two wire identities for one \
                     retired slot",
                );
                expected
                    .tombstones
                    .push((slot.ordinal, slot.name.clone().filter(|n| !n.is_empty())));
            }
            // A typl declaration nested in an interface is vocabulary, emitted
            // at package level; it is not an interaction and carries no ordinal.
            Some(_) | None => {}
        }
    }

    kinds.fallible_queries += expected.transport.len();
    for (_, tombstone) in &expected.tombstones {
        match tombstone {
            Some(_) => kinds.named_tombstones += 1,
            None => kinds.nameless_tombstones += 1,
        }
    }
    for (_, timing) in &expected.timing {
        if timing.mode == "strict-periodic" {
            kinds.strict_periodic_timings += 1;
        }
        if timing.min_us.is_none() || timing.max_us.is_none() {
            kinds.half_open_timings += 1;
        }
        if timing.default_applied {
            kinds.defaulted_timings += 1;
        }
    }
    for stub in &expected.contracts {
        if stub.uses_result {
            kinds.result_reading_clauses += 1;
        }
    }

    expected
}

/// One interaction's contract clauses, in source order.
fn stubs_of(contracts: &[v2::Contract]) -> impl Iterator<Item = ContractStub> + '_ {
    contracts.iter().map(|contract| ContractStub {
        id: contract.observer_id.clone(),
        kind: contract_kind(contract.kind),
        uses_result: contract.uses_result,
    })
}

fn expected_timing(spec: &v2::Timing) -> Timing {
    Timing {
        mode: timing_mode(spec.mode),
        min_us: spec.min_us.clone(),
        max_us: spec.max_us.clone(),
        default_applied: spec.default_applied,
    }
}

/// Every interface a package generates faces for: the named `interface`s, then
/// the inline shape of every service that declares one (ridl §14.5).
fn expected_package(ir: &v2::Package, kinds: &mut Kinds) -> BTreeMap<String, Expected> {
    let mut out: BTreeMap<String, Expected> = BTreeMap::new();
    let mut insert = |key: String, expected: Expected| {
        assert!(
            out.insert(key.clone(), expected).is_none(),
            "two interfaces reduce to the canonical key `{key}`; the name normalisation \
             would merge them and hide a difference between them",
        );
    };

    // `Package::shapes` — a named interface and a service's inline shape alike,
    // named first, in the order both backends emit them.
    for shape in ir.shapes() {
        let key = match shape.service {
            // Both backends prefix an inline shape's generated name with
            // `Service` and follow it with the dotted segments; the canonical
            // key is that same construction, reduced.
            Some(_) => {
                kinds.inline_service_shapes += 1;
                canonical_key(&format!("Service{name}", name = shape.name))
            }
            None => canonical_key(shape.name),
        };
        insert(key, expected_interface(shape.name, shape.interface, kinds));
    }
    out
}

// ==========================================================================
// Reading the generated Rust.
// ==========================================================================

/// The text of a `///` doc line, with the leading marker removed.
fn rust_doc(line: &str) -> Option<&str> {
    let text = line.trim_start().strip_prefix("///")?;
    Some(text.strip_prefix(' ').unwrap_or(text))
}

/// The `(interaction, ordinal)` a generated Rust doc line records.
///
/// The Rust backend names the ordinal in prose, and names the interaction by
/// its **source** spelling in backticks — `currentSpeed`, not the `current_speed`
/// the method is called. Both faces are covered: a consumer member reads
/// ``signal `currentSpeed` — ordinal 1 (ridl §4).`` and a provider member reads
/// ``Publishes `currentSpeed` — signal ordinal 1 (ridl §4).``
///
/// The leading verb is matched against a closed list so an authored doc comment
/// that happens to contain an em dash cannot be mistaken for an ordinal record.
fn rust_ordinal(line: &str) -> Option<(String, u32)> {
    const HEADS: [&str; 8] = [
        "signal ",
        "event ",
        "command ",
        "query ",
        "fixed ",
        "Publishes ",
        "Raises ",
        "Handles ",
    ];
    let text = rust_doc(line)?;
    let (head, tail) = text.split_once(" — ")?;
    if !HEADS.iter().any(|prefix| head.starts_with(prefix)) {
        return None;
    }
    let name = head.split('`').nth(1)?;
    let digits = match tail.strip_prefix("ordinal ") {
        Some(rest) => rest,
        None => tail.split_once(" ordinal ")?.1,
    };
    let ordinal = leading_number(digits)?;
    Some((name.to_string(), ordinal))
}

/// The `(ordinal, retired name)` a generated tombstone line records, in either
/// form: ``reserved ordinal 2 (`legacyWheelPhase`) — retired, never reused.``
/// or `reserved ordinal 4 — retired, never reused.`
fn rust_tombstone(line: &str) -> Option<(u32, Option<String>)> {
    let rest = rust_doc(line)?.strip_prefix("reserved ordinal ")?;
    let ordinal = leading_number(rest)?;
    let name = rest.split('`').nth(1).map(str::to_string);
    Some((ordinal, name))
}

fn leading_number(text: &str) -> Option<u32> {
    let digits: String = text.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

/// Collapses every run of whitespace to one space, so a `prettyplease`-formatted
/// literal can be read without depending on where the formatter broke lines.
fn collapse_whitespace(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut pending = false;
    for ch in text.chars() {
        if ch.is_whitespace() {
            pending = true;
            continue;
        }
        if pending && !out.is_empty() {
            out.push(' ');
        }
        pending = false;
        out.push(ch);
    }
    out
}

/// The lines of a generated block: everything between the declaration line at
/// index `start` and the first line equal to `close` at column zero.
fn block_lines<'a>(lines: &[&'a str], start: usize, close: &str) -> Vec<&'a str> {
    lines[start + 1..]
        .iter()
        .take_while(|line| **line != close)
        .copied()
        .collect()
}

/// The `///` lines immediately above index `start`, in source order. The
/// interface's own doc — which is where a tombstone is recorded — sits here,
/// possibly with attributes (`#[allow(...)]`, `#[deprecated]`) interleaved.
fn rust_leading_docs<'a>(lines: &[&'a str], start: usize) -> Vec<&'a str> {
    let mut docs = Vec::new();
    for index in (0..start).rev() {
        let line = lines[index];
        if line.starts_with("///") || line.starts_with("#[") {
            docs.push(line);
        } else {
            break;
        }
    }
    docs.reverse();
    docs
}

fn parse_rust(source: &str) -> Rendered {
    let lines: Vec<&str> = source.lines().collect();
    let mut rendered = Rendered::default();

    for (index, line) in lines.iter().enumerate() {
        if let Some((visibility, rest)) = rust_declaration(line, "trait ") {
            let Some((key, face)) = face_of(rest) else {
                continue;
            };
            let body = if rest.ends_with("{}") {
                Vec::new()
            } else {
                block_lines(&lines, index, "}")
            };
            rendered.faces.insert(
                (key, face),
                rust_face(visibility, &rust_leading_docs(&lines, index), &body),
            );
            continue;
        }
        if let Some((visibility, rest)) = rust_declaration(line, "const ") {
            // `NAME_TIMING: &[(&str, TimingConst)] = &[`
            let Some((name, _)) = rest.split_once(": ") else {
                continue;
            };
            let body = rust_const_body(&lines, index, rest);
            if let Some(stem) = name.strip_suffix("_TIMING") {
                rendered
                    .timing
                    .insert(canonical_key(stem), (visibility, parse_rust_timing(&body)));
            } else if let Some(stem) = name.strip_suffix("_CONTRACTS") {
                rendered.contracts.insert(
                    canonical_key(stem),
                    (visibility, parse_rust_contracts(&body)),
                );
            }
        }
    }

    rendered
}

/// A top-level Rust declaration of `keyword`, with its visibility.
fn rust_declaration<'a>(line: &'a str, keyword: &str) -> Option<(Visibility, &'a str)> {
    for (prefix, visibility) in [
        ("pub(crate) ", Visibility::PackagePrivate),
        ("pub ", Visibility::Public),
    ] {
        if let Some(rest) = line
            .strip_prefix(prefix)
            .and_then(|r| r.strip_prefix(keyword))
        {
            return Some((visibility, rest));
        }
    }
    None
}

/// The `(canonical key, face)` of a generated face name, or `None` when the
/// name is not a face — the emitted vocabulary (`SignalHandle<T>`, `RidlStream`)
/// and every typl declaration land here.
fn face_of(rest: &str) -> Option<(String, Face)> {
    let name = rest
        .strip_suffix(" {}")
        .or_else(|| rest.strip_suffix(" {"))?;
    for (suffix, face) in [("Consumer", Face::Consumer), ("Provider", Face::Provider)] {
        if let Some(stem) = name.strip_suffix(suffix) {
            return Some((canonical_key(stem), face));
        }
    }
    None
}

fn rust_face(visibility: Visibility, docs: &[&str], body: &[&str]) -> RenderedFace {
    let mut ordinals = Vec::new();
    let mut transport = Vec::new();
    let mut current: Option<String> = None;
    for line in body {
        if let Some((name, ordinal)) = rust_ordinal(line) {
            current = Some(name.clone());
            ordinals.push((name, ordinal));
        } else if let Some(identity) =
            rust_doc(line).and_then(|t| t.strip_prefix("transport identity: "))
        {
            let owner = current
                .clone()
                .expect("a transport identity follows the ordinal line of its own interaction");
            transport.push((owner, identity.to_string()));
        }
    }
    RenderedFace {
        visibility,
        ordinals,
        transport,
        tombstones: docs
            .iter()
            .filter_map(|line| rust_tombstone(line))
            .collect(),
    }
}

/// The body of a generated `const` array: the inline form when the declaration
/// closes on its own line, the block form otherwise.
fn rust_const_body(lines: &[&str], index: usize, rest: &str) -> String {
    match rest.split_once(" = &[") {
        Some((_, tail)) => match tail.strip_suffix("];") {
            Some(inline) => inline.to_string(),
            None => block_lines(lines, index, "];").join("\n"),
        },
        None => String::new(),
    }
}

fn parse_rust_timing(body: &str) -> Vec<(String, Timing)> {
    const MARK: &str = "TimingConst {";
    let flat = collapse_whitespace(body);
    let mut out = Vec::new();
    let mut rest = flat.as_str();
    while let Some(at) = rest.find(MARK) {
        let name =
            last_quoted(&rest[..at]).expect("a generated timing entry names its interaction");
        let after = &rest[at + MARK.len()..];
        let end = after
            .find('}')
            .expect("a generated TimingConst literal closes");
        out.push((name, rust_timing_fields(&after[..end])));
        rest = &after[end + 1..];
    }
    out
}

fn rust_timing_fields(fields: &str) -> Timing {
    let mut timing = Timing {
        mode: String::new(),
        min_us: None,
        max_us: None,
        default_applied: false,
    };
    for field in fields.split(',') {
        let Some((key, value)) = field.split_once(':') else {
            continue;
        };
        let value = value.trim();
        match key.trim() {
            "mode" => {
                timing.mode = match value {
                    "TimingMode::StrictPeriodic" => "strict-periodic",
                    "TimingMode::Range" => "range",
                    other => panic!("the Rust backend emitted an unknown timing mode `{other}`"),
                }
                .to_string();
            }
            "min_us" => timing.min_us = rust_optional_number(value),
            "max_us" => timing.max_us = rust_optional_number(value),
            "default_applied" => timing.default_applied = value == "true",
            _ => {}
        }
    }
    timing
}

/// `Some(10000)` becomes `Some("10000")`; `None` stays absent. The two are the
/// Rust spelling of a present and an absent bound.
fn rust_optional_number(value: &str) -> Option<String> {
    value
        .strip_prefix("Some(")
        .and_then(|rest| rest.strip_suffix(')'))
        .map(str::to_string)
}

/// The contents of the last double-quoted string in `text`.
fn last_quoted(text: &str) -> Option<String> {
    let close = text.rfind('"')?;
    let open = text[..close].rfind('"')?;
    Some(text[open + 1..close].to_string())
}

/// Every `ContractStub` literal in a generated contracts const. The fields are
/// emitted in a fixed order, so `id` and `kind` are read by anchored prefix
/// rather than by search — a contract's `source` is arbitrary author text and
/// must never be able to look like a field.
fn parse_rust_contracts(body: &str) -> Vec<ContractStub> {
    const MARK: &str = "ContractStub {";
    let flat = collapse_whitespace(body);
    let mut out = Vec::new();
    for chunk in flat.split(MARK).skip(1) {
        let after_id = chunk
            .strip_prefix(" id: \"")
            .expect("a generated ContractStub opens with its id");
        let (id, after) = after_id
            .split_once('"')
            .expect("a generated contract id is a string literal");
        let (kind, _) = after
            .strip_prefix(", kind: ContractKind::")
            .expect("a generated ContractStub records its kind after its id")
            .split_once(',')
            .expect("a generated contract kind is followed by another field");
        let uses_result = chunk
            .rsplit_once("uses_result: ")
            .expect("a generated ContractStub records uses_result")
            .1
            .starts_with("true");
        out.push(ContractStub {
            id: id.to_string(),
            kind: kind.trim().to_ascii_lowercase(),
            uses_result,
        });
    }
    out
}

// ==========================================================================
// Reading the generated TypeScript.
// ==========================================================================

fn parse_typescript(source: &str) -> Rendered {
    let lines: Vec<&str> = source.lines().collect();
    let mut rendered = Rendered::default();

    for (index, line) in lines.iter().enumerate() {
        if let Some((visibility, rest)) = ts_declaration(line, "interface ") {
            let Some((key, face)) = face_of(rest) else {
                continue;
            };
            let body = if rest.ends_with("{}") {
                Vec::new()
            } else {
                block_lines(&lines, index, "}")
            };
            rendered.faces.insert(
                (key, face),
                ts_face(visibility, &ts_leading_jsdoc(&lines, index), &body),
            );
            continue;
        }
        if let Some((visibility, rest)) = ts_declaration(line, "const ") {
            // `vehicleStatusTiming = {` / `vehicleStatusContracts = [`
            let Some((name, _)) = rest.split_once(" = ") else {
                continue;
            };
            if let Some(stem) = name.strip_suffix("Timing") {
                let body = ts_const_body(&lines, index, rest, "} as const;");
                rendered.timing.insert(
                    canonical_key(stem),
                    (
                        visibility,
                        body.iter().filter_map(|l| ts_timing(l)).collect(),
                    ),
                );
            } else if let Some(stem) = name.strip_suffix("Contracts") {
                let body = ts_const_body(&lines, index, rest, "] as const;");
                rendered.contracts.insert(
                    canonical_key(stem),
                    (
                        visibility,
                        body.iter().filter_map(|l| ts_contract(l)).collect(),
                    ),
                );
            }
        }
    }

    rendered
}

/// A top-level TypeScript declaration of `keyword`, with its visibility. An
/// unexported declaration is package-private: a generated module is one file, so
/// a name without `export` is unreachable from outside it.
fn ts_declaration<'a>(line: &'a str, keyword: &str) -> Option<(Visibility, &'a str)> {
    if let Some(rest) = line
        .strip_prefix("export ")
        .and_then(|r| r.strip_prefix(keyword))
    {
        return Some((Visibility::Public, rest));
    }
    line.strip_prefix(keyword)
        .map(|rest| (Visibility::PackagePrivate, rest))
}

/// The JSDoc block immediately above index `start`, as its content lines.
fn ts_leading_jsdoc<'a>(lines: &[&'a str], start: usize) -> Vec<&'a str> {
    if start == 0 || lines[start - 1].trim() != "*/" {
        return Vec::new();
    }
    let mut docs = Vec::new();
    for index in (0..start - 1).rev() {
        if lines[index].trim_start().starts_with("/**") {
            break;
        }
        docs.push(lines[index]);
    }
    docs.reverse();
    docs
}

/// The text of one JSDoc content line, with the ` * ` gutter removed.
fn jsdoc_text(line: &str) -> &str {
    line.trim_start().strip_prefix("* ").unwrap_or_default()
}

fn ts_face(visibility: Visibility, docs: &[&str], body: &[&str]) -> RenderedFace {
    let mut ordinals = Vec::new();
    let mut transport = Vec::new();
    let mut index = 0;
    while index < body.len() {
        if !body[index].trim_start().starts_with("/**") {
            index += 1;
            continue;
        }
        let mut ordinal = None;
        let mut identity = None;
        index += 1;
        while index < body.len() && body[index].trim() != "*/" {
            let text = jsdoc_text(body[index]);
            if let Some(value) = text.strip_prefix("@ordinal ") {
                ordinal = leading_number(value.trim());
            } else if let Some(value) = text.strip_prefix("@transportIdentity ") {
                identity = Some(value.trim().to_string());
            }
            index += 1;
        }
        // Past the `*/`, the member this doc belongs to.
        index += 1;
        let Some(name) = body.get(index).and_then(|line| ts_member_name(line)) else {
            continue;
        };
        if let Some(ordinal) = ordinal {
            ordinals.push((name.clone(), ordinal));
        }
        if let Some(identity) = identity {
            transport.push((name, identity));
        }
    }
    RenderedFace {
        visibility,
        ordinals,
        transport,
        tombstones: docs
            .iter()
            .filter_map(|line| ts_tombstone(jsdoc_text(line)))
            .collect(),
    }
}

/// The name of a generated face member: `currentSpeed: SignalHandle<…>;`,
/// `setGear(…): Promise<void>;`, or `readonly softwareVersion: …;`.
fn ts_member_name(line: &str) -> Option<String> {
    let text = line.trim_start();
    let text = text.strip_prefix("readonly ").unwrap_or(text);
    let name: String = text
        .chars()
        .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '_' || *ch == '$')
        .collect();
    if name.is_empty() {
        return None;
    }
    let separator = text[name.len()..].chars().next()?;
    (separator == ':' || separator == '(').then_some(name)
}

/// ``@reserved ordinal 2 (`legacyWheelPhase`) — retired, never reused.``
fn ts_tombstone(text: &str) -> Option<(u32, Option<String>)> {
    let rest = text.strip_prefix("@reserved ordinal ")?;
    let ordinal = leading_number(rest)?;
    Some((ordinal, rest.split('`').nth(1).map(str::to_string)))
}

/// The body of a generated TypeScript const, one entry per line.
fn ts_const_body<'a>(lines: &[&'a str], index: usize, rest: &str, close: &str) -> Vec<&'a str> {
    if rest.ends_with("as const;") {
        return Vec::new();
    }
    block_lines(lines, index, close)
}

/// `currentSpeed: { mode: 'strict-periodic', minUs: 10000n, maxUs: 10000n, defaultApplied: false },`
fn ts_timing(line: &str) -> Option<(String, Timing)> {
    let (name, rest) = line.trim().split_once(": { mode: '")?;
    let (mode, rest) = rest.split_once('\'')?;
    let (min, rest) = rest.strip_prefix(", minUs: ")?.split_once(", maxUs: ")?;
    let (max, rest) = rest.split_once(", defaultApplied: ")?;
    Some((
        name.to_string(),
        Timing {
            mode: mode.to_string(),
            min_us: ts_optional_number(min),
            max_us: ts_optional_number(max),
            default_applied: rest.starts_with("true"),
        },
    ))
}

/// `10000n` becomes `Some("10000")`; `undefined` stays absent. The bigint suffix
/// is TypeScript's only exact form for a microsecond count.
fn ts_optional_number(value: &str) -> Option<String> {
    value.strip_suffix('n').map(str::to_string)
}

/// `{ id: 'X.y.require[0]', kind: 'require', source: '…', signals: [], params: [], usesResult: false },`
fn ts_contract(line: &str) -> Option<ContractStub> {
    let text = line.trim();
    let (id, rest) = text.strip_prefix("{ id: '")?.split_once('\'')?;
    let (kind, _) = rest.strip_prefix(", kind: '")?.split_once('\'')?;
    let uses_result = text.rsplit_once("usesResult: ")?.1.starts_with("true");
    Some(ContractStub {
        id: id.to_string(),
        kind: kind.to_string(),
        uses_result,
    })
}

// ==========================================================================
// Compiling the corpus.
// ==========================================================================

/// One package of a corpus entry: the checked IR and both backends' output.
struct Generated {
    package: String,
    ir: v2::Package,
    rust: String,
    typescript: String,
}

/// Compiles the corpus entry rooted at `entry`, or returns `None` when it is a
/// diagnostic showcase.
///
/// An entry crafted to carry errors lowers partially, so code generation over it
/// is not meaningful — the same gate `corpus.rs` applies before it snapshots
/// generated code. The entries this leaves are exactly the full-pipeline golden,
/// and the count assertions at the end of this file are what keep that set from
/// silently shrinking to nothing.
fn generated_entry(entry: &Path) -> Option<Vec<Generated>> {
    let mut db = RidlDatabase::default();
    let std = std_package(&mut db);
    let loaded = load_workspace(&mut db, entry).expect("a corpus entry loads");
    let workspace = loaded.workspace;
    if loaded.diagnostics.iter().any(is_error) {
        return None;
    }

    let packages: Vec<Package> = workspace.packages(&db).clone();
    let mut out = Vec::new();
    for pkg in &packages {
        let files: Vec<InputFile> = pkg.files(&db).clone();
        if files
            .iter()
            .any(|file| !parse_file(&db, *file).errors().is_empty())
        {
            return None;
        }
        let resolution = resolve_package(&db, workspace, *pkg, std);
        let checked = check_package(&db, workspace, *pkg, std);
        if resolution.diagnostics.iter().any(is_error) || checked.diagnostics.iter().any(is_error) {
            return None;
        }
        out.push(Generated {
            package: pkg.name(&db).clone(),
            rust: ridl_backend_rust::generate(&checked.ir)
                .expect("a clean entry's IR generates Rust")
                .rust_source,
            typescript: ridl_backend_ts::generate(&checked.ir)
                .expect("a clean entry's IR generates TypeScript")
                .source,
            ir: checked.ir,
        });
    }
    Some(out)
}

fn is_error(diagnostic: &ridl_core::diag::Diagnostic) -> bool {
    diagnostic.severity == Severity::Error
}

/// Every corpus entry directory, by name, in a stable order.
fn corpus_entries() -> Vec<(String, PathBuf)> {
    let root = Path::new("tests/corpus");
    let mut entries: Vec<(String, PathBuf)> = std::fs::read_dir(root)
        .expect("the corpus directory is readable")
        .map(|entry| entry.expect("a corpus directory entry is readable").path())
        .filter(|path| path.join("ridl.toml").is_file())
        .map(|path| {
            let name = path
                .file_name()
                .expect("a corpus entry has a directory name")
                .to_string_lossy()
                .into_owned();
            (name, path)
        })
        .collect();
    entries.sort();
    entries
}

// ==========================================================================
// The comparison.
// ==========================================================================

/// How much this guard actually checked, so an empty corpus cannot pass. Each
/// figure is accumulated once per backend, so it is twice the number of IR facts.
#[derive(Default)]
struct Counts {
    interfaces: usize,
    ordinals: usize,
    tombstones: usize,
    transport: usize,
    contracts: usize,
    timing: usize,
}

/// Which **constructs** the corpus still exercises, counted once per IR fact.
///
/// [`Counts`] alone is not enough, and the gap is the one this whole file exists
/// to close. A quantity floor counts facts, not kinds: drop the `internal`
/// keyword from `internal-shape.ridl` and every total is unchanged, so the guard
/// stays green over a corpus that no longer contains the construct that
/// motivated #160. The same holds for the nameless `reserved <n>` form, a
/// `default_applied` bound, a half-open range, a `fixed`, and an inline service
/// shape — several of which have exactly one carrier today, so one edit retires
/// them.
///
/// That is measured, not argued. With the loop below neutralised and every other
/// assertion in this file left intact — the face, timing and contract
/// comparisons, the collision check, [`MUST_COVER`] and all six quantity floors —
/// **eight of the ten constructs below can be removed from the corpus and this
/// test still passes**. Only losing every inline service shape or every fallible
/// query is visible to anything else, and each of those is visible only because
/// it drags a quantity to zero.
///
/// Each construct therefore gets a floor of its own, and **every floor here was
/// verified by removing the construct from the corpus and watching this test go
/// red**. That constraint is why the list is not longer. Three counters that
/// belong to the same family were considered and left out, because a floor that
/// cannot fail is the thing this epic has spent its time deleting:
///
/// All three arguments rest on the lowering rather than on the specification.
/// That is deliberate: `ridl-sem`'s `check_internal_exposure` carries recorded
/// debt (issue #161) that a public `service` publishing an `internal` interface
/// draws no diagnostic today, so a rule the checker does not yet enforce is the
/// wrong thing to hang an unfalsifiability claim on.
///
/// - *a public interface* — `ridl-sem`'s `lower_service_inline` hardcodes an
///   inline shape's `Interface.visibility` to `VISIBILITY_UNSPECIFIED`, which
///   [`Visibility::of`] reads as public. The counter therefore cannot reach zero
///   while `inline_service_shapes` is non-zero, and that is itself a floor
///   below.
/// - *a named `interface`* — implied by `package_private_interfaces`, since only
///   a named `interface` can carry `internal`: the same hardcoded
///   `VISIBILITY_UNSPECIFIED` means an inline shape never contributes to the
///   package-private counter. Zeroing this counter therefore zeroes that one,
///   whose floor is first in the list and fires first.
/// - *a range timing* — implied by `half_open_timings`. `ridl-sem` fills both
///   bounds for a strict period, so an absent bound implies a non-strict mode,
///   and the only other resolved mode is `range` — `TIMING_MODE_UNSPECIFIED` is
///   refused by both backends before this floor is reached.
#[derive(Default)]
struct Kinds {
    package_private_interfaces: usize,
    inline_service_shapes: usize,
    named_tombstones: usize,
    nameless_tombstones: usize,
    fixed: usize,
    fallible_queries: usize,
    strict_periodic_timings: usize,
    half_open_timings: usize,
    defaulted_timings: usize,
    rpc_bound_timings: usize,
    result_reading_clauses: usize,
}

/// Asserts that one backend's generated output carries exactly the facts the IR
/// fixes, for every interface of one package.
fn assert_backend(
    backend: &str,
    entry: &str,
    package: &str,
    expected: &BTreeMap<String, Expected>,
    rendered: &Rendered,
) -> Counts {
    let mut counts = Counts::default();
    let where_ = format!("{backend} backend, corpus entry `{entry}`, package `{package}`");

    // No face without an interface behind it, and none missing. Set equality
    // rather than containment: a face the IR does not describe is as much a
    // finding as a face the IR describes and the backend did not emit.
    let expected_keys: BTreeSet<&String> = expected.keys().collect();
    let rendered_faces: BTreeSet<&String> = rendered.faces.keys().map(|(key, _)| key).collect();
    assert_eq!(
        rendered_faces, expected_keys,
        "{where_}: the generated faces and the IR's interfaces are different sets",
    );
    // The same equality for the two metadata constants. Looking these up by key
    // would accept a constant with no interface behind it, where an unexplained
    // face already fails — an inconsistency with nothing behind it.
    assert_eq!(
        rendered.timing.keys().collect::<BTreeSet<_>>(),
        expected_keys,
        "{where_}: the generated timing constants and the IR's interfaces are different sets",
    );
    assert_eq!(
        rendered.contracts.keys().collect::<BTreeSet<_>>(),
        expected_keys,
        "{where_}: the generated contract constants and the IR's interfaces are different sets",
    );

    for (key, want) in expected {
        counts.interfaces += 1;
        let identity = &want.identity;
        let at = format!("{where_}, interface `{identity}`");

        for (face, ordinals, tombstones) in [
            (Face::Consumer, &want.consumer, want.tombstones.as_slice()),
            (Face::Provider, &want.provider, &[][..]),
        ] {
            let got = rendered
                .faces
                .get(&(key.clone(), face))
                .unwrap_or_else(|| panic!("{at}: the {} is not generated", face.label()));

            assert_eq!(
                &got.ordinals,
                ordinals,
                "{at}: the {} does not carry the IR's interaction ordinals. \
                 Every interaction's `Decl.ordinal` is its wire identity (ridl §11); a \
                 tag-based transport derives its numeric ids from it and `ridl diff` \
                 rejects any change that shifts or reuses one.",
                face.label(),
            );
            counts.ordinals += got.ordinals.len();

            assert_eq!(
                got.tombstones,
                tombstones,
                "{at}: the {} does not carry the IR's tombstones. A `reserved` slot holds \
                 its ordinal for ever; without the record, a retired wire identity reads as \
                 a free slot.",
                face.label(),
            );
            counts.tombstones += got.tombstones.len();

            assert_eq!(
                got.visibility,
                want.visibility,
                "{at}: the {} has the wrong visibility. An `internal` interface is \
                 package-private in full (ADR-0002 §8, ADR-0008 decision 7) — the keyword \
                 must not hide the declaration and publish its API.",
                face.label(),
            );
        }

        // The consumer face carries the transport identity in both backends.
        let consumer = &rendered.faces[&(key.clone(), Face::Consumer)];
        assert_eq!(
            &consumer.transport, &want.transport,
            "{at}: the consumer face does not carry the IR's transport identities. The \
             identity is derived from the interface's own identity, the interaction ordinal \
             and both arm references (ADR-0008 decision 4); a registry keys a wire contract \
             on that string, so every consumer of it has to agree.",
        );
        counts.transport += consumer.transport.len();

        // The provider face may repeat it (TypeScript does, Rust does not).
        // Presence is not asserted — that would freeze a doc-comment placement
        // choice — but every pair recorded there must be one the IR derives.
        // Pairs, not values: an identity that is correct for some other
        // interaction of the same interface is still the wrong identity here.
        for recorded in &rendered.faces[&(key.clone(), Face::Provider)].transport {
            assert!(
                want.transport.contains(recorded),
                "{at}: the provider face records `{}` as the transport identity of `{}`, \
                 which is not what the IR derives for it: {:?}",
                recorded.1,
                recorded.0,
                want.transport,
            );
        }

        let (timing_visibility, timing) = rendered
            .timing
            .get(key)
            .unwrap_or_else(|| panic!("{at}: the timing constant is not generated"));
        assert_eq!(
            timing, &want.timing,
            "{at}: the timing constant does not carry the IR's resolved bounds. Timing is \
             resolved at compile time into exact microsecond bounds (ridl §9.1, ADR-0008 \
             decision 12).",
        );
        assert_eq!(
            *timing_visibility, want.visibility,
            "{at}: the timing constant has the wrong visibility (ADR-0008 decision 7)",
        );
        counts.timing += timing.len();

        let (contracts_visibility, contracts) = rendered
            .contracts
            .get(key)
            .unwrap_or_else(|| panic!("{at}: the contracts constant is not generated"));
        assert_eq!(
            contracts, &want.contracts,
            "{at}: the contracts constant does not carry the IR's observer stubs. `id` is the \
             stub identity a runtime installs against and `usesResult` cannot be recovered \
             from `source` (E2.5).",
        );
        assert_eq!(
            *contracts_visibility, want.visibility,
            "{at}: the contracts constant has the wrong visibility — this is the generated \
             name the first report of the pre-#160 defect overlooked",
        );
        counts.contracts += contracts.len();
    }

    counts
}

// ==========================================================================
// The guard.
// ==========================================================================

/// The corpus entries that must be covered. Naming them is what makes a
/// disappearance visible: an entry that stops compiling clean silently drops out
/// of [`generated_entry`], and without this the guard would keep passing over
/// whatever was left.
const MUST_COVER: [&str; 2] = ["services-workspace", "veh-cluster"];

/// Both backends carry every interaction-layer fact the IR fixes.
///
/// The three defects in this file's header would each fail here: a transport
/// identity keyed on anything but the interface's IR identity, an `internal`
/// interface generated public, and a missing ordinal or tombstone.
#[test]
fn both_backends_carry_every_ir_fact_the_corpus_fixes() {
    let mut covered: BTreeSet<String> = BTreeSet::new();
    let mut totals = Counts::default();
    let mut kinds = Kinds::default();

    for (name, path) in corpus_entries() {
        let Some(packages) = generated_entry(&path) else {
            continue;
        };
        for generated in &packages {
            let expected = expected_package(&generated.ir, &mut kinds);
            if !expected.is_empty() {
                covered.insert(name.clone());
            }
            for (backend, source) in [
                ("rust", parse_rust(&generated.rust)),
                ("typescript", parse_typescript(&generated.typescript)),
            ] {
                let counts = assert_backend(backend, &name, &generated.package, &expected, &source);
                totals.interfaces += counts.interfaces;
                totals.ordinals += counts.ordinals;
                totals.tombstones += counts.tombstones;
                totals.transport += counts.transport;
                totals.contracts += counts.contracts;
                totals.timing += counts.timing;
            }
        }
    }

    // Anti-vacuity. Every assertion above is inside a loop, so an empty corpus,
    // a corpus whose interaction-layer entries stopped compiling clean, or a
    // parser that silently found no faces would all pass without these. The
    // floors are set below the counts at the time of writing, so the guard
    // tolerates an interface being removed but not the corpus being emptied.
    for entry in MUST_COVER {
        assert!(
            covered.contains(entry),
            "the corpus entry `{entry}` carries interfaces and must be covered by this guard, \
             but it produced none — it no longer compiles clean, or its interfaces are gone. \
             Covered: {covered:?}",
        );
    }
    // The kind floors. Counted once per IR fact, not once per backend, because
    // the question they answer is about the corpus rather than about how much
    // was compared: does the corpus still contain the construct at all? Each
    // message names the construct and where it lives, so a corpus edit that
    // retires the last carrier says what to restore rather than what number
    // moved.
    for (present, construct, why) in [
        (
            kinds.package_private_interfaces,
            "an `internal` interface (`veh-cluster/cluster/internal-shape.ridl`)",
            "the construct that motivated #160. Without one in the corpus, every visibility \
             assertion in this file compares Public against Public and the pre-#160 defect \
             replays green",
        ),
        (
            kinds.inline_service_shapes,
            "a service with an inline shape (ridl §14.5)",
            "the other interaction store, and the only one whose identity and generated type \
             name differ — which is exactly what the transport-identity defect turned on",
        ),
        (
            kinds.named_tombstones,
            "a named tombstone, `reserved someName`",
            "the form that carries a retired name for `ridl diff` to report",
        ),
        (
            kinds.nameless_tombstones,
            "a nameless tombstone, `reserved <n>` (`veh-cluster/cluster/evolution.ridl`)",
            "the other grammatical form (typl §7.4); it lowers with `name: None` and is the \
             half a backend is likelier to drop",
        ),
        (
            kinds.fixed,
            "a `fixed` interaction",
            "the one kind the face rule treats asymmetrically. Without a `fixed` the consumer \
             and provider ordinal lists are identical, and the face rule is asserted over two \
             copies of the same list",
        ),
        (
            kinds.fallible_queries,
            "a query with an inline `T | E` return",
            "the only carrier of a synthesized transport identity (ADR-0008 decision 4)",
        ),
        (
            kinds.strict_periodic_timings,
            "a strict-periodic timing, `@Xms`",
            "one of the two timing modes (ridl §9)",
        ),
        (
            kinds.half_open_timings,
            "a half-open timing range",
            "the only carrier of an absent bound — Rust's `None` against TypeScript's \
             `undefined`, one of the six normalisations this file declares",
        ),
        (
            kinds.defaulted_timings,
            "a timing the compiler resolved from the configured default",
            "the only carrier of `default_applied: true`. \"Untimed\" does not exist beyond \
             the parser (ridl §9.1), so this flag is how a reader tells a written bound from \
             a resolved one — and exactly one interaction in the corpus carries it",
        ),
        (
            kinds.rpc_bound_timings,
            "a command or query with declared RPC bounds (ADR-0015)",
            "the only carrier of a timing entry on an RPC kind. Without one, the timing \
             comparison never reaches a command's or query's table row, and a backend that \
             dropped the two RPC arms from its timing walk would stay green",
        ),
        (
            kinds.result_reading_clauses,
            "a contract clause that reads `result`",
            "the only carrier of `uses_result: true`. The flag cannot be recovered from \
             `source`, so without one the field is asserted as false everywhere and a backend \
             hardcoding false is green",
        ),
    ] {
        assert!(
            present > 0,
            "the corpus no longer contains {construct}, so this guard no longer covers it. \
             That is {why}. Restore the construct, or delete this floor deliberately and say \
             in the commit message what stopped being guarded.",
        );
    }

    // Each figure counts both backends, so it is twice the number of IR facts.
    assert!(
        totals.interfaces >= 16,
        "the guard checked only {} interfaces across both backends; the corpus carries far \
         more, so the parse found nothing",
        totals.interfaces,
    );
    assert!(
        totals.ordinals >= 160,
        "the guard checked only {} interaction ordinals across both backends",
        totals.ordinals,
    );
    assert!(
        totals.tombstones >= 10,
        "the guard checked only {} tombstones across both backends",
        totals.tombstones,
    );
    assert!(
        totals.transport >= 6,
        "the guard checked only {} transport identities across both backends",
        totals.transport,
    );
    assert!(
        totals.contracts >= 24,
        "the guard checked only {} contract stubs across both backends",
        totals.contracts,
    );
    assert!(
        totals.timing >= 36,
        "the guard checked only {} timing entries across both backends",
        totals.timing,
    );
}
