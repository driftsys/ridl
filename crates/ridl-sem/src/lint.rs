//! The ridl lint pass (docs/ROADMAP.md story E2.10a): four advisory codes over
//! a package's interaction declarations, run from [`crate::check::check_package`]
//! once lowering has finished.
//!
//! The lints are **ordinary coded diagnostics** — warnings and infos on the
//! same channel as every other check, so the CLI and the LSP surface them
//! through the existing diagnostic pipeline. There is deliberately no lint
//! driver and no configuration surface in E2: a lint that cannot be switched
//! off is a lint whose message has to be worth reading.
//!
//! | Code     | Rule                                                     | Severity |
//! | -------- | -------------------------------------------------------- | -------- |
//! | RIDL-404 | query named like a mutation (ridl §7.2)                  | warning  |
//! | RIDL-405 | one error type shared across unrelated domains (§10.1)   | info     |
//! | RIDL-406 | payload re-declaring envelope metadata (ridl §3.1)       | info     |
//! | RIDL-308 | named result union in return position (general form §6.1) | warning  |
//!
//! The E2.10 story also lists an "alias not required" lint. It needs no work
//! here: TYPL-008 (an import alias without an actual collision, warning) has
//! shipped from the resolver since E1, so the row is already covered.

use std::collections::BTreeMap;
use std::collections::BTreeSet;

use ridl_core::db::InputFile;
use ridl_core::diag::DiagCode;
use ridl_syntax::ast::{self, AstNode, Definition};

use crate::check::Checker;
use crate::resolve::{Symbol, SymbolKind, resolve_package, significant_text, source_file};

/// The number of distinct interfaces an error type must serve before RIDL-405
/// calls it "shared across unrelated failure domains". Two interfaces are a
/// deliberate pairing; three start to look like one catch-all error type
/// standing in for several failure domains. The threshold lives here, not in a
/// configuration file — E2 ships no lint configuration surface.
const SHARED_ERROR_INTERFACE_THRESHOLD: usize = 3;

/// The verbs that name a mutation (ridl §7.2). A query whose name starts with
/// one of these followed by an upper-case letter is probably a `command`.
const MUTATION_VERBS: [&str; 6] = ["set", "reset", "clear", "apply", "write", "update"];

/// The payload field names that duplicate envelope metadata (ridl §3.1): the
/// envelope already carries a publication timestamp and a sequence number, so a
/// payload repeating either one is storing transport metadata as contract data.
const ENVELOPE_FIELDS: [&str; 8] = [
    "timestamp",
    "time",
    "seq",
    "seqNo",
    "sequence",
    "sequenceNumber",
    "frameCounter",
    "frameNo",
];

/// One lintable interaction scope: a declared `interface` or the inline shape
/// of a `service` (ridl §14.0, §14.5). The name identifies the failure domain
/// RIDL-405 counts distinct occurrences of.
struct Scope {
    /// Index into the package's file list — the checker's `current_file`.
    file: usize,
    name: String,
    members: Vec<ast::InterfaceMember>,
}

/// Runs the four ridl lints over `files`, appending their diagnostics to
/// `checker`.
///
/// RIDL-405 counts across the whole package, so the scopes are collected once
/// and walked twice: the first walk tallies which interfaces use each error
/// type, the second emits every lint with that tally in hand.
pub(crate) fn lint_package(checker: &mut Checker<'_>, files: &[InputFile]) {
    let scopes = collect_scopes(checker, files);
    let shared_errors = tally_error_domains(checker, &scopes);

    // The pass walks files in its own order, so it borrows `current_file` and
    // puts it back: leaving the checker pointing at whichever file happened to
    // be last would hand a stale span origin to anything that ran afterwards.
    let entry_file = checker.current_file;
    for scope in &scopes {
        checker.current_file = scope.file;
        for member in &scope.members {
            match member {
                ast::InterfaceMember::Query(query) => {
                    lint_query_name(checker, query);
                    lint_query_return(checker, query, &shared_errors);
                }
                ast::InterfaceMember::Signal(signal) => {
                    lint_envelope_payload(checker, "signal", signal.payload());
                }
                ast::InterfaceMember::Event(event) => {
                    lint_envelope_payload(checker, "event", event.payload());
                }
                _ => {}
            }
        }
    }
    checker.current_file = entry_file;
}

// ==========================================================================
// Scope collection
// ==========================================================================

/// The interaction scopes of the package, in file then declaration order.
///
/// Interfaces are filtered through the resolver's first-wins rule so a losing
/// duplicate is not linted twice; a `service` contributes only when it carries
/// an inline shape, since a named reference points at an interface already
/// collected in its own right.
fn collect_scopes(checker: &Checker<'_>, files: &[InputFile]) -> Vec<Scope> {
    let mut scopes = Vec::new();
    for (index, file) in files.iter().enumerate() {
        let source = source_file(checker.db, *file);
        for shape in source.shapes() {
            // The first-wins filter applies to `interface` declarations only:
            // a service's dotted name lives in the catalog namespace, not the
            // type namespace the resolver arbitrates.
            if let ast::InterfaceShape::Interface(def) = &shape
                && !checker.is_winner(*file, def)
            {
                continue;
            }
            scopes.push(Scope {
                file: index,
                name: shape.identity().unwrap_or_default(),
                members: shape.members().collect(),
            });
        }
    }
    scopes
}

// ==========================================================================
// RIDL-404 — a query named like a mutation (ridl §7.2)
// ==========================================================================

/// Flags a query whose name starts with a mutating verb followed by an
/// upper-case letter (`setGear`, `resetCounters`). The trailing upper-case
/// letter is what separates the verb prefix from a word that merely starts the
/// same way — `settings`, `updated`, `clearance` are not mutations.
///
/// The heuristic is spec-mandated: ridl §7.2 names `set…` and `reset…`
/// specifically. It is still a heuristic, and a residual false-positive class
/// is inherent to it — a compound whose first word is a domain **noun** reads
/// as a verb to the matcher (`setPoint`, `resetReason`, `updateAvailable`,
/// `writeProtectEnabled`, `applyForce` are all read-only and all fire). §7.2
/// calls these "**probable** commands", so the message hedges to match and
/// names the noun case explicitly: a warning that overstates on a legitimate
/// domain name teaches readers to ignore the whole code.
fn lint_query_name(checker: &mut Checker<'_>, query: &ast::QueryDef) {
    let Some(name) = query.name() else {
        return;
    };
    let Some(token) = name.ident_token() else {
        return;
    };
    let text = token.text().to_string();
    let Some(verb) = mutation_verb(&text) else {
        return;
    };
    let range = name.syntax().text_range();
    checker.warning(
        DiagCode::RIDL_404,
        range,
        format!(
            "query `{text}` is named like a mutation (`{verb}…`) — queries should be read-only or \
             idempotent, so if it mutates state it belongs to `command` (ridl §7.2). If the name \
             is a domain noun rather than a verb (`setPoint`, `resetReason`), ignore this."
        ),
    );
}

/// The mutating verb `name` starts with, when the next character is upper-case.
fn mutation_verb(name: &str) -> Option<&'static str> {
    MUTATION_VERBS.into_iter().find(|verb| {
        name.strip_prefix(verb)
            .and_then(|rest| rest.chars().next())
            .is_some_and(char::is_uppercase)
    })
}

// ==========================================================================
// RIDL-405 / RIDL-308 — query return position
// ==========================================================================

/// The interfaces each error type serves, keyed by the error type's canonical
/// reference. Only queries contribute: a `command` has no return position, so
/// it declares no failure arm.
fn tally_error_domains(
    checker: &Checker<'_>,
    scopes: &[Scope],
) -> BTreeMap<String, BTreeSet<String>> {
    let mut domains: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for scope in scopes {
        for member in &scope.members {
            let ast::InterfaceMember::Query(query) = member else {
                continue;
            };
            let Some(return_type) = query.return_type() else {
                continue;
            };
            if let Some((_, error)) = fallible_arms(checker, &return_type) {
                domains.entry(error).or_default().insert(scope.name.clone());
            }
        }
    }
    domains
}

/// Emits the two return-position lints for one query: RIDL-308 when the return
/// is a named result union, RIDL-405 when its failure arm is shared across
/// enough interfaces.
fn lint_query_return(
    checker: &mut Checker<'_>,
    query: &ast::QueryDef,
    shared_errors: &BTreeMap<String, BTreeSet<String>>,
) {
    let Some(return_type) = query.return_type() else {
        return;
    };

    // RIDL-308: the named spelling stays legal typl data (a result union is
    // storable in a struct, a log, a snapshot), but in return position the
    // inline `T | E` is canonical — one way to say it where it matters
    // (general form §6.1, ADR-0008 decision 13).
    if let Some(path) = return_type.type_ref()
        && let Some(symbol) = checker.lookup_path(&path)
        && let Some((ok, error)) = result_union_arms(checker, &symbol)
    {
        let written = significant_text(path.syntax());
        checker.warning(
            DiagCode::RIDL_308,
            path.syntax().text_range(),
            format!(
                "query returns the named result union `{written}` — in return position the inline \
                 spelling is canonical: write `{ok} | {error}` (general form §6.1)"
            ),
        );
    }

    // RIDL-405: one error type answering for several unrelated failure
    // domains. The tally covers the package's own interfaces; the count is
    // quoted so the reader can judge the heuristic rather than trust it.
    let Some((_, error)) = fallible_arms(checker, &return_type) else {
        return;
    };
    let Some(interfaces) = shared_errors.get(&error) else {
        return;
    };
    if interfaces.len() < SHARED_ERROR_INTERFACE_THRESHOLD {
        return;
    }
    let range = return_type.syntax().text_range();
    checker.info(
        DiagCode::RIDL_405,
        range,
        format!(
            "error type `{error}` is the failure arm of queries in {count} interaction scopes \
             ({list}) — one error type spanning unrelated failure domains makes each caller \
             handle failures it cannot cause (ridl §10.1)",
            count = interfaces.len(),
            list = interfaces
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>()
                .join(", "),
        ),
    );
}

/// The `(success, error)` canonical arm references a query return declares, for
/// both spellings of a fallible return: the inline `T | E` (general form §6.1)
/// and the named result union (typl §10.2). Every other return shape — a plain
/// type, a named tuple, a stream — yields `None`.
fn fallible_arms(checker: &Checker<'_>, return_type: &ast::ReturnType) -> Option<(String, String)> {
    if let Some(fallible) = return_type.fallible_type() {
        let ok = checker.lookup_path(&fallible.ok()?)?;
        let error = checker.lookup_path(&fallible.err()?)?;
        // Arm-order and arm-kind mistakes are RIDL-303's jurisdiction, already
        // reported at lowering; the lints read only a well-formed pair.
        if ok.is_error || !error.is_error {
            return None;
        }
        return Some((checker.canonical_ref(&ok), checker.canonical_ref(&error)));
    }
    let symbol = checker.lookup_path(&return_type.type_ref()?)?;
    result_union_arms(checker, &symbol)
}

/// The `(success, error)` canonical arm references of a named result union, or
/// `None` when `symbol` does not name one.
///
/// The classification itself stays with the checker's [`Checker::union_is_result`]
/// — the same predicate lowering uses to set `UnionDef.is_result`, so the lint
/// and the IR can never disagree about what a result union is. This function
/// only picks the two arms back out once that gate has passed.
fn result_union_arms(checker: &Checker<'_>, symbol: &Symbol) -> Option<(String, String)> {
    if symbol.kind != SymbolKind::Union || !checker.union_is_result(symbol) {
        return None;
    }
    let Some(Definition::Union(decl)) = checker.find_definition(symbol) else {
        return None;
    };
    let package = checker.package_handle(&symbol.package)?;
    let resolution = resolve_package(checker.db, checker.ws, package, checker.std);
    let mut ok = None;
    let mut error = None;
    for child in decl.syntax().children() {
        let Some(arm) = ast::UnionArm::cast(child) else {
            continue;
        };
        let Some(path) = arm.type_ref() else { continue };
        let Some(arm_symbol) = checker.lookup_path_in(&resolution, &path) else {
            continue;
        };
        let reference = checker.canonical_ref(&arm_symbol);
        if arm_symbol.is_error {
            error = Some(reference);
        } else {
            ok = Some(reference);
        }
    }
    Some((ok?, error?))
}

// ==========================================================================
// RIDL-406 — a payload re-declaring the envelope (ridl §3.1)
// ==========================================================================

/// Flags a `signal` or `event` whose payload struct declares a field that
/// duplicates envelope metadata.
///
/// The scope is deliberately narrow: only a payload draws the lint, because
/// only a payload rides an envelope. The same struct returned from a query —
/// including a `<T>` stream return, where the envelope timestamps *delivery*
/// rather than occurrence — is exactly the legitimate case ridl §3.1 carves
/// out, and stays silent.
///
/// The matched set covers both halves of the envelope, so the message's
/// legitimacy clause has to as well: §3.1 spells out the domain-time exception
/// (a `FaultEvent.timestamp` recording when the fault *occurred*), and the
/// counter names carry the same exception one step over — a domain sequence
/// number counts the facts themselves, not the publications carrying them.
/// Every name matched here has a stated legitimate reading.
///
/// The diagnostic points at the payload reference rather than at the offending
/// field, because the struct may live in another file or another package while
/// a diagnostic's span must stay inside the package being checked.
fn lint_envelope_payload(
    checker: &mut Checker<'_>,
    keyword: &str,
    payload: Option<ast::FieldType>,
) {
    let Some(ast::FieldType::Path(path)) = payload else {
        return;
    };
    let Some(symbol) = checker.lookup_path(&path) else {
        return;
    };
    let range = path.syntax().text_range();
    for field in envelope_fields(checker, &symbol) {
        checker.info(
            DiagCode::RIDL_406,
            range,
            format!(
                "{keyword} payload `{payload}` declares `{field}`, which the envelope already \
                 carries — every publication is delivered with a timestamp and a sequence number, \
                 so the payload need not re-declare them (ridl §3.1). Domain metadata distinct \
                 from transport metadata is legitimate: keep the field if it records when the \
                 fact occurred rather than when it was published, or if it counts the fact \
                 itself rather than the publication carrying it.",
                payload = symbol.name,
            ),
        );
    }
}

/// The envelope-duplicating field names declared by the struct `symbol` names,
/// in declaration order. A non-struct payload declares no fields.
fn envelope_fields(checker: &Checker<'_>, symbol: &Symbol) -> Vec<String> {
    if symbol.kind != SymbolKind::Struct {
        return Vec::new();
    }
    let Some(Definition::Struct(decl)) = checker.find_definition(symbol) else {
        return Vec::new();
    };
    decl.members()
        .filter_map(|member| match member {
            ast::StructMember::Field(field) => field.name()?.ident_token(),
            ast::StructMember::Reserved(_) => None,
        })
        .map(|token| token.text().to_string())
        .filter(|name| ENVELOPE_FIELDS.contains(&name.as_str()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::check::{CheckedPackage, check_package};
    use ridl_core::db::RidlDatabase;
    use ridl_core::package::{Package, PackageOrigin, Workspace};
    use ridl_core::std_lib::std_package;
    use std::collections::BTreeMap;

    /// Checks a single-file `.ridl` package and returns its diagnostics.
    fn check_ridl(text: &str) -> CheckedPackage {
        check_ridl_files(&[("app.ridl", text)])
    }

    /// Checks one package assembled from several named `.ridl` files, in the
    /// order given.
    fn check_ridl_files(files: &[(&str, &str)]) -> CheckedPackage {
        let mut db = RidlDatabase::default();
        let std = std_package(&mut db);
        let inputs: Vec<InputFile> = files
            .iter()
            .map(|(name, text)| InputFile::new(&db, (*name).to_string(), (*text).to_string()))
            .collect();
        let pkg = Package::new(
            &db,
            "app".to_string(),
            inputs,
            PackageOrigin::WorkspaceMember,
            BTreeMap::new(),
            None,
        );
        let ws = Workspace::new(&db, vec![pkg], BTreeMap::new());
        check_package(&db, ws, pkg, std)
    }

    /// The diagnostic codes, in order.
    fn codes(checked: &CheckedPackage) -> Vec<&str> {
        checked
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code.as_str())
            .collect()
    }

    /// The messages of the diagnostics carrying `code`.
    fn messages<'a>(checked: &'a CheckedPackage, code: &str) -> Vec<&'a str> {
        checked
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code.as_str() == code)
            .map(|diagnostic| diagnostic.message.as_str())
            .collect()
    }

    const PRELUDE: &str = "\
package app

type Speed : km/h [0.0..300.0 step 0.5]

enum GearPosition {
  PARK  = 0
  DRIVE = 1
}
";

    // --- RIDL-404 ---------------------------------------------------------

    #[test]
    fn ridl_404_flags_a_mutation_named_query() {
        let checked = check_ridl(&format!(
            "{PRELUDE}
interface Drive {{
  query setGear(position: GearPosition): Speed @[..50ms]
}}
"
        ));
        assert_eq!(
            codes(&checked),
            vec!["RIDL-404"],
            "{:?}",
            checked.diagnostics
        );
        let message = messages(&checked, "RIDL-404")[0];
        assert!(message.contains("setGear"), "got: {message}");
        assert!(message.contains("command"), "got: {message}");
    }

    /// Every verb of the §7.2 set fires, and the same verbs on a `command`
    /// stay silent — `command setGear` is the idiomatic spelling the lint
    /// steers toward, so flagging it would be self-defeating.
    #[test]
    fn ridl_404_covers_the_verb_set_and_spares_commands() {
        let checked = check_ridl(&format!(
            "{PRELUDE}
interface Drive {{
  query setGear(position: GearPosition): Speed @[..50ms]
  query resetTrip(position: GearPosition): Speed @[..50ms]
  query clearFaults(position: GearPosition): Speed @[..50ms]
  query applyBrake(position: GearPosition): Speed @[..50ms]
  query writeConfig(position: GearPosition): Speed @[..50ms]
  query updateMap(position: GearPosition): Speed @[..50ms]
  command setSpeed(target: Speed) @[..50ms]
}}
"
        ));
        assert_eq!(
            codes(&checked),
            vec!["RIDL-404"; 6],
            "{:?}",
            checked.diagnostics
        );
    }

    /// A name that merely starts with the same letters is not a mutation: the
    /// verb must be followed by an upper-case letter.
    #[test]
    fn ridl_404_is_silent_on_a_word_that_only_starts_with_a_verb() {
        let checked = check_ridl(&format!(
            "{PRELUDE}
interface Drive {{
  query settings(position: GearPosition): Speed @[..50ms]
  query clearance(position: GearPosition): Speed @[..50ms]
  query getGear(position: GearPosition): GearPosition @[..50ms]
}}
"
        ));
        assert!(codes(&checked).is_empty(), "{:?}", checked.diagnostics);
    }

    /// The heuristic's residual false-positive class, pinned deliberately.
    ///
    /// All five names are read-only and idiomatic in this domain, and all five
    /// fire: the matcher cannot tell a verb from a compound whose first word is
    /// a domain noun, and ridl §7.2 mandates the `set…`/`reset…` prefixes
    /// anyway. What the lint owes them is honest wording — §7.2 says
    /// "**probable** commands" — so the message must stay conditional and must
    /// keep naming the domain-noun case. This test fails if either is dropped.
    #[test]
    fn ridl_404_reads_as_advisory_on_domain_nouns() {
        let checked = check_ridl(&format!(
            "{PRELUDE}
interface Ecu {{
  query setPoint(): Speed @[..50ms]
  query resetReason(): GearPosition @[..50ms]
  query updateAvailable(): GearPosition @[..50ms]
  query writeProtectEnabled(): GearPosition @[..50ms]
  query applyForce(): Speed @[..50ms]
}}
"
        ));
        assert_eq!(
            codes(&checked),
            vec!["RIDL-404"; 5],
            "{:?}",
            checked.diagnostics
        );
        for message in messages(&checked, "RIDL-404") {
            assert!(
                message.contains("if it mutates state"),
                "the claim must stay conditional, got: {message}"
            );
            assert!(
                message.contains("domain noun") && message.contains("ignore this"),
                "the escape hatch must name the noun case, got: {message}"
            );
        }
    }

    // --- RIDL-405 ---------------------------------------------------------

    const DIAG_VOCABULARY: &str = "\
package app

type Speed : km/h [0.0..300.0 step 0.5]

struct FaultPage {
  count : integer [0..64]
}

error enum DiagError {
  STORAGE_BUSY  = 0
  ACCESS_DENIED = 1
}
";

    #[test]
    fn ridl_405_is_silent_at_two_interfaces() {
        let checked = check_ridl(&format!(
            "{DIAG_VOCABULARY}
interface Cluster {{
  query faults(): FaultPage | DiagError @[..50ms]
}}

interface Powertrain {{
  query faults(): FaultPage | DiagError @[..50ms]
}}
"
        ));
        assert!(codes(&checked).is_empty(), "{:?}", checked.diagnostics);
    }

    #[test]
    fn ridl_405_fires_at_three_interfaces() {
        let checked = check_ridl(&format!(
            "{DIAG_VOCABULARY}
interface Cluster {{
  query faults(): FaultPage | DiagError @[..50ms]
}}

interface Powertrain {{
  query faults(): FaultPage | DiagError @[..50ms]
}}

interface Infotainment {{
  query faults(): FaultPage | DiagError @[..50ms]
}}
"
        ));
        assert_eq!(
            codes(&checked),
            vec!["RIDL-405"; 3],
            "{:?}",
            checked.diagnostics
        );
        let message = messages(&checked, "RIDL-405")[0];
        assert!(message.contains("DiagError"), "got: {message}");
        // "interaction scopes", not "interfaces": a scope may be a service's
        // inline shape, which is an interface without being one by name.
        assert!(message.contains("3 interaction scopes"), "got: {message}");
        assert!(message.contains("Cluster"), "got: {message}");
    }

    /// The tally counts distinct interfaces, not queries: three queries in one
    /// interface are one failure domain.
    #[test]
    fn ridl_405_counts_interfaces_not_queries() {
        let checked = check_ridl(&format!(
            "{DIAG_VOCABULARY}
interface Cluster {{
  query faults(): FaultPage | DiagError @[..50ms]
  query archive(): FaultPage | DiagError @[..50ms]
  query live(): FaultPage | DiagError @[..50ms]
}}
"
        ));
        assert!(codes(&checked).is_empty(), "{:?}", checked.diagnostics);
    }

    /// A named result union counts toward the tally exactly as the inline
    /// spelling does — the error type is shared either way. Each of the three
    /// queries also earns its own RIDL-308.
    #[test]
    fn ridl_405_counts_named_result_unions_too() {
        let checked = check_ridl(&format!(
            "{DIAG_VOCABULARY}
union FaultPageResult {{
  page : FaultPage
  err  : DiagError
}}

interface Cluster {{
  query faults(): FaultPageResult @[..50ms]
}}

interface Powertrain {{
  query faults(): FaultPageResult @[..50ms]
}}

interface Infotainment {{
  query faults(): FaultPage | DiagError @[..50ms]
}}
"
        ));
        assert_eq!(
            codes(&checked),
            vec!["RIDL-308", "RIDL-405", "RIDL-308", "RIDL-405", "RIDL-405"],
            "{:?}",
            checked.diagnostics
        );
    }

    // --- RIDL-406 ---------------------------------------------------------

    const FAULT_EVENT: &str = "\
package app

type Speed : km/h [0.0..300.0 step 0.5]

struct FaultEvent {
  code      : integer [0..65535]
  timestamp : Timestamp
}
";

    /// The §3.1 scoping in one test: the same struct is silent as a query
    /// stream return — the envelope of a streamed history reply timestamps
    /// delivery, not occurrence — and draws the lint as a signal payload.
    #[test]
    fn ridl_406_scopes_to_payloads_not_stream_returns() {
        let silent = check_ridl(&format!(
            "{FAULT_EVENT}
interface Diagnostics {{
  query streamFaults(): <FaultEvent> @[..50ms]
}}
"
        ));
        assert!(codes(&silent).is_empty(), "{:?}", silent.diagnostics);

        let flagged = check_ridl(&format!(
            "{FAULT_EVENT}
interface Diagnostics {{
  signal lastFault : FaultEvent @10ms
}}
"
        ));
        assert_eq!(
            codes(&flagged),
            vec!["RIDL-406"],
            "{:?}",
            flagged.diagnostics
        );
        let message = messages(&flagged, "RIDL-406")[0];
        assert!(message.contains("FaultEvent"), "got: {message}");
        assert!(message.contains("timestamp"), "got: {message}");
        // The legitimate exception travels with the message, so the reader who
        // genuinely records domain time knows to keep the field.
        assert!(message.contains("occurred"), "got: {message}");
    }

    /// Every name the lint matches must have a stated legitimate reading: the
    /// counter half of the envelope needs its own escape hatch, not only the
    /// time half. A genuine domain sequence number is the case in point.
    #[test]
    fn ridl_406_states_a_legitimate_case_for_counters_too() {
        let checked = check_ridl(
            "\
package app

struct TestRun {
  sequence : integer [0..1024]
  passed   : boolean
}

interface Harness {
  event ran : TestRun @[10ms..1s]
}
",
        );
        assert_eq!(
            codes(&checked),
            vec!["RIDL-406"],
            "{:?}",
            checked.diagnostics
        );
        let message = messages(&checked, "RIDL-406")[0];
        assert!(
            message.contains("counts the fact itself"),
            "a matched counter name needs a counter-shaped exception, got: {message}"
        );
    }

    #[test]
    fn ridl_406_covers_event_payloads_and_the_whole_field_set() {
        let checked = check_ridl(
            "\
package app

struct Sample {
  seqNo        : integer [0..255]
  frameCounter : integer [0..255]
  value        : integer [0..255]
}

interface Telemetry {
  event sampled : Sample @[10ms..100ms]
}
",
        );
        assert_eq!(
            codes(&checked),
            vec!["RIDL-406"; 2],
            "{:?}",
            checked.diagnostics
        );
    }

    /// A payload whose fields carry no envelope metadata is silent.
    #[test]
    fn ridl_406_is_silent_on_a_clean_payload() {
        let checked = check_ridl(
            "\
package app

struct DoorPayload {
  sensorId : integer [0..15]
  isOpen   : boolean
}

interface Doors {
  event doorOpened : DoorPayload @[50ms..500ms]
}
",
        );
        assert!(codes(&checked).is_empty(), "{:?}", checked.diagnostics);
    }

    // --- RIDL-308 ---------------------------------------------------------

    #[test]
    fn ridl_308_flags_a_named_result_union_return() {
        let checked = check_ridl(&format!(
            "{DIAG_VOCABULARY}
union FaultPageResult {{
  page : FaultPage
  err  : DiagError
}}

interface Diagnostics {{
  query getFaultPage(): FaultPageResult @[..50ms]
}}
"
        ));
        assert_eq!(
            codes(&checked),
            vec!["RIDL-308"],
            "{:?}",
            checked.diagnostics
        );
        let message = messages(&checked, "RIDL-308")[0];
        assert!(message.contains("FaultPageResult"), "got: {message}");
        // The message spells the canonical replacement out.
        assert!(message.contains("FaultPage | DiagError"), "got: {message}");
    }

    #[test]
    fn ridl_308_is_silent_on_the_inline_spelling() {
        let checked = check_ridl(&format!(
            "{DIAG_VOCABULARY}
interface Diagnostics {{
  query getFaultPage(): FaultPage | DiagError @[..50ms]
}}
"
        ));
        assert!(codes(&checked).is_empty(), "{:?}", checked.diagnostics);
    }

    /// A named result union stays legal typl data — declaring one, and using
    /// it in a struct field, draws nothing. Only return position is steered.
    #[test]
    fn ridl_308_leaves_a_result_union_used_as_data_alone() {
        let checked = check_ridl(&format!(
            "{DIAG_VOCABULARY}
union FaultPageResult {{
  page : FaultPage
  err  : DiagError
}}

struct AuditRecord {{
  outcome : FaultPageResult
}}
"
        ));
        assert!(codes(&checked).is_empty(), "{:?}", checked.diagnostics);
    }

    // --- inline service shapes -------------------------------------------

    /// A `service`'s inline shape is an interface (ridl §14.5), so its
    /// interactions are linted the same way.
    #[test]
    fn lints_reach_inline_service_shapes() {
        let checked = check_ridl(&format!(
            "{PRELUDE}
service veh.drive {{
  query setGear(position: GearPosition): Speed @[..50ms]
}}
"
        ));
        assert_eq!(
            codes(&checked),
            vec!["RIDL-404"],
            "{:?}",
            checked.diagnostics
        );
    }

    /// A duplicate `interface` name in one package is linted once, not twice:
    /// the scope walk runs each `interface` declaration through the resolver's
    /// first-wins rule (ADR-0007 decision 6), so the loser — already reported
    /// as TYPL-009 and excluded from the lowering — does not draw a second copy
    /// of every advisory its body would earn.
    #[test]
    fn a_losing_duplicate_interface_is_not_linted_twice() {
        let winner = format!(
            "{PRELUDE}
interface Drive {{
  query setGear(position: GearPosition): Speed @[..50ms]
}}
"
        );
        let loser = "package app
interface Drive {
  query setGear(position: GearPosition): Speed @[..50ms]
}
";
        let checked = check_ridl_files(&[("a.ridl", &winner), ("b.ridl", loser)]);
        assert_eq!(
            codes(&checked)
                .iter()
                .filter(|code| **code == "RIDL-404")
                .count(),
            1,
            "the losing re-declaration must not draw its own RIDL-404: {:?}",
            checked.diagnostics,
        );
    }
}
