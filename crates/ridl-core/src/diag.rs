//! The coded diagnostic model every compiler pass emits (docs/ROADMAP.md epic
//! E1.10, ADR-0004 §5, ADR-0007 decision 2).
//!
//! A [`Diagnostic`] is a first-class homegrown value — a stable [`DiagCode`], a
//! [`Severity`], a message, a primary source [`Span`], secondary [`Label`]s, and
//! optional [`FixIt`]s — held as the single source of truth and accumulated in a
//! `Vec`, never modeled as an error return (ADR-0004 §5). The struct maps two
//! ways: to a terminal renderer for the CLI ([`render`](render::render), over
//! `codespan-reporting`) and, in a later epic, to LSP `Diagnostic` for editors.
//!
//! # Namespaces (ADR-0007 decision 2)
//!
//! Codes are grouped by hundreds and never renumbered or reused. Four
//! namespaces are in play across the family, one catalogue each:
//!
//! - `FORM-…` — the shared family grammar: lexical `0xx`, parse `1xx`, and the
//!   general form §4.3 attribute rules. Named after the general form's own
//!   "shared form namespace". [`FORM_CATALOG`].
//! - `TYPL-…` — typl semantic rules, defined by the typl reference §16.
//!   [`TYPL_CATALOG`].
//! - `RIDL-…` — ridl interaction rules, defined by the ridl reference §16.
//!   [`RIDL_CATALOG`].
//! - `MANI-…` — manifest, lockfile, cache, and fetch: the manifest `0xx` codes
//!   (E1.5) and the distribution `1xx` codes (E1.6). [`MANI_CATALOG`].
//!
//! This module is the SSOT the error index (E4.2) reads. Every code is declared
//! once, by [`diag_codes!`], which expands one entry into both the `DiagCode`
//! constant and its catalogue row — a code with no entry cannot be written
//! (ADR-0008 decision 21). A catalogue lists a code even when no pass emits it
//! yet.
//!
//! # The [`FileId`] bridge
//!
//! A [`Span`] locates its range inside a file identified by an interned
//! [`FileId`]. [`SourceMap`] issues those ids: a pass that holds a file's path
//! and text calls [`SourceMap::file_id`] to obtain the id it stamps into its
//! spans. The [`SourceMap`] is the only bridge between a diagnostic and the
//! source text the renderer needs.

use rowan::TextRange;
use serde::{Serialize, Serializer};

pub mod render;

pub use render::render;

/// A stable diagnostic code, e.g. `"FORM-101"` or `"TYPL-108"` (ADR-0007
/// decision 2). The empty string means "no code yet" — a diagnostic that has
/// not been assigned a catalogue code renders as a plain message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct DiagCode(pub &'static str);

impl DiagCode {
    /// The code string.
    pub fn as_str(&self) -> &'static str {
        self.0
    }

    /// Whether this is the sentinel "no code yet" value.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The sentinel for a diagnostic that carries no catalogue code yet. The
    /// unknown-type-name check uses it until a later task rehomes it (typl §16
    /// defines no code for it); the Rust backend's codegen error uses it too.
    pub const NONE: DiagCode = DiagCode("");
}

/// Declares every diagnostic code exactly once (ADR-0008 decision 21).
///
/// One entry expands to both the `DiagCode` constant and the [`CatalogEntry`]
/// that names it, so a code with no catalogue entry cannot be written: there is
/// no second list to forget.
///
/// The macro also generates [`ALL_CATALOGS`], which the guards read to find
/// every catalogue. That makes it invocable only once **per module** — a second
/// invocation beside this one redefines the constant and the crate stops
/// compiling. It does **not** make it invocable once per crate: a second
/// invocation inside a child module of `diag` compiles, and its catalogue is
/// invisible to `ALL_CATALOGS` here. What covers that case is
/// `codes_written_as_string_literals_are_all_catalogued`, and it covers it for a
/// structural reason rather than by luck — `$code:literal` guarantees every code
/// the macro declares is spelled as a literal in a `.rs` file, so a catalogue
/// the guards cannot see still puts its codes where the scan can.
///
/// This replaces the pair of hand-maintained arrays `FORM_CATALOG` and
/// `MANI_CATALOG` carried until E2 close-out, each guarded by a test that
/// compared it against a *second hand-written list inside the test*. That pair
/// checked that two lists agreed; it never checked either against the codes
/// actually declared, so a constant added to neither compiled and turned nothing
/// red. `RIDL` and `TYPL` had no catalogue at all.
///
/// The remedy available in `tools/diff` — make the guard an exhaustive `match`
/// and let the compiler enforce totality — does not exist here: `DiagCode` is a
/// newtype over `&'static str`, not an enum, so there is no variant set to match
/// on. Declaring the constant and its entry from one line is the shape that
/// works for a newtype.
macro_rules! diag_codes {
    (
        $(
            $(#[$catalog_doc:meta])*
            $catalog:ident {
                $(
                    $(#[$code_doc:meta])*
                    $konst:ident = $code:literal, $severity:ident,
                        $summary:literal;
                )+
            }
        )+
    ) => {
        impl DiagCode {
            $($(
                $(#[$code_doc])*
                pub const $konst: DiagCode = DiagCode($code);
            )+)+
        }

        $(
            $(#[$catalog_doc])*
            pub const $catalog: &[CatalogEntry] = &[
                $(CatalogEntry {
                    code: DiagCode::$konst,
                    severity: Severity::$severity,
                    summary: $summary,
                },)+
            ];
        )+

        /// Every catalogue this module declares, each paired with its constant's
        /// name. The error index (E4.2) reads this rather than naming the four
        /// catalogues one at a time, and so do the guards below.
        pub const ALL_CATALOGS: &[(&str, &[CatalogEntry])] = &[
            $((stringify!($catalog), $catalog),)+
        ];

        /// Each constant's name paired with the code string it expands to, so a
        /// guard can check the two agree. Generated rather than written down —
        /// a hand-written copy would be the shadow this macro exists to remove.
        #[cfg(test)]
        const CODE_CONSTANT_NAMES: &[(&str, &str)] = &[
            $($((stringify!($konst), $code),)+)+
        ];
    };
}

diag_codes! {
    /// The FORM catalogue (ADR-0007 decision 2): lexical `0xx`, parse `1xx`, and
    /// the attribute-semantics codes 106-108 the checker emits for the general
    /// form §4.3 allow-list (E2 task 5). Every FORM code is listed even when no
    /// pass emits it yet, so the error index has one authoritative source. FORM
    /// diagnostics are all errors.
    FORM_CATALOG {
        /// Invalid character.
        FORM_001 = "FORM-001", Error,
            "invalid character";

        /// Unterminated string literal.
        FORM_002 = "FORM-002", Error,
            "unterminated string literal";

        /// Unterminated regex literal.
        FORM_003 = "FORM-003", Error,
            "unterminated regex literal";

        /// Unterminated block comment.
        FORM_004 = "FORM-004", Error,
            "unterminated block comment";

        /// Leading zeros in an integer literal.
        FORM_005 = "FORM-005", Error,
            "leading zeros in integer literal";

        /// Expected a specific token.
        FORM_101 = "FORM-101", Error,
            "expected a specific token";

        /// Unexpected token.
        FORM_102 = "FORM-102", Error,
            "unexpected token";

        /// Unclosed delimiter.
        FORM_103 = "FORM-103", Error,
            "unclosed delimiter";

        /// Missing `package` declaration.
        FORM_104 = "FORM-104", Error,
            "missing `package` declaration";

        /// Reserved word used as an identifier.
        FORM_105 = "FORM-105", Error,
            "reserved word used as an identifier";

        /// Unknown attribute key — not a key the general form §4.3 table defines.
        FORM_106 = "FORM-106", Error,
            "unknown attribute key";

        /// Attribute key not allowed on this declaration kind (general form §4.3).
        FORM_107 = "FORM-107", Error,
            "attribute key not allowed on this declaration kind";

        /// Duplicate attribute key in one `[ ]` block (general form §4.3).
        FORM_108 = "FORM-108", Error,
            "duplicate attribute key in one block";
    }

    /// The typl catalogue (ADR-0008 decision 21): every `TYPL-` code declared in
    /// this module, with the severity the typl reference §16 tables classify it
    /// at. Six codes the reference documents are absent because no constant
    /// declares them and no pass emits them — TYPL-107, TYPL-112, TYPL-205, and
    /// the three `@labels` assurance codes TYPL-401 to TYPL-403. That inventory
    /// is recorded in issue #172; closing it means minting the constants, which
    /// is a change to what the compiler declares, not a catalogue edit.
    TYPL_CATALOG {
        /// More than one `package` declaration in a single file (typl §16.1).
        /// Emitted by the package loader (E1.3).
        TYPL_001 = "TYPL-001", Error,
            "more than one `package` declaration in a file";

        /// Package name does not mirror the directory path relative to the
        /// manifest root (typl §16.1, ADR-0002 §1). Emitted by the package loader
        /// (E1.3); single-file mode is exempt.
        TYPL_002 = "TYPL-002", Error,
            "package name does not mirror the directory path";

        /// Wildcard, relative, or re-exporting import (typl §16.1, ADR-0002 §2).
        /// Emitted by the resolver (E1.4).
        TYPL_003 = "TYPL-003", Error,
            "wildcard, relative, or re-exporting import";

        /// Circular package imports (typl §16.1, ADR-0002 §6). Emitted by the
        /// resolver (E1.4) from a depth-first walk over package import edges.
        TYPL_004 = "TYPL-004", Error,
            "circular package imports";

        /// A public declaration exposes an `internal` type in its fields, arms,
        /// backing, or a range-bound constant (typl §3.3, §16.1). Emitted by the
        /// checker (E1.7b) over every top-level declaration, the ridl `interface`
        /// and `service` included: an interaction payload, parameter, return arm,
        /// or stream element is an exposure position exactly as a struct field is.
        /// A `service` naming an `internal` interface is RIDL-143 instead.
        TYPL_005 = "TYPL-005", Error,
            "a public declaration exposes an `internal` type";

        /// Conflicting imports without an alias (typl §16.1, ADR-0002 §2).
        /// Emitted by the resolver (E1.4).
        TYPL_006 = "TYPL-006", Error,
            "conflicting imports without an alias";

        /// Unused import (typl §16.1). Emitted by the resolver (E1.4) as a warning.
        TYPL_007 = "TYPL-007", Warning,
            "unused import";

        /// Import alias without an actual collision (typl §16.1, ADR-0002 §2).
        /// Emitted by the resolver (E1.4) as a warning.
        TYPL_008 = "TYPL-008", Warning,
            "import alias without an actual collision";

        /// Duplicate definition of the same name in a package (typl §16.1).
        TYPL_009 = "TYPL-009", Error,
            "duplicate definition of the same name in a package";

        /// `integer` without a range constraint (typl §16.2). Warning.
        TYPL_101 = "TYPL-101", Warning,
            "`integer` without a range constraint";

        /// `float` without both a range and a `step` (typl §16.2). Warning.
        TYPL_102 = "TYPL-102", Warning,
            "`float` without both a range and a `step`";

        /// `string`/`bytes` without explicit bounds — the default `[0..256]` is
        /// applied (typl §4.4–§4.5, §16.2). Warning.
        TYPL_103 = "TYPL-103", Warning,
            "`string`/`bytes` without explicit bounds";

        /// Range `min > max` (typl §16.2).
        TYPL_104 = "TYPL-104", Error,
            "range `min > max`";

        /// `step` type mismatch, non-positive, or larger than the range
        /// (typl §16.2). Also borrowed by the checker (E1.7b) for a range bound
        /// that references a non-numeric constant, a malformed bound const for
        /// which §16.2 defines no dedicated code.
        TYPL_105 = "TYPL-105", Error,
            "`step` type mismatch, non-positive, or larger than the range";

        /// Invalid regex syntax in a `match` constraint or a regex `const`
        /// (typl §16.2). Validated with the `regress` ECMA-262 engine (ADR-0007
        /// decision 10). Emitted by the checker (E1.7b).
        TYPL_106 = "TYPL-106", Error,
            "invalid regex syntax in `match` or a regex `const`";

        /// `const` value violates its declared type constraints (typl §16.2).
        TYPL_108 = "TYPL-108", Error,
            "`const` value violates its declared type constraints";

        /// Init (`= value`) incompatible with the type/field constraints
        /// (typl §16.2).
        TYPL_109 = "TYPL-109", Error,
            "init `= value` incompatible with the type or field constraints";

        /// Unknown or malformed UCUM unit expression (typl §16.2).
        TYPL_110 = "TYPL-110", Error,
            "unknown or malformed UCUM unit expression";

        /// Integer range bound (or enumset bit position) outside the `int64`
        /// domain (typl §4.2, §16.2).
        TYPL_111 = "TYPL-111", Error,
            "integer range bound outside the `int64` domain";

        /// Type has no derivable init value and no declared `= value`
        /// (typl §5.8, §16.2). Info — escalated to an error only by consumers
        /// that require an init (e.g. a ridl signal payload).
        TYPL_115 = "TYPL-115", Info,
            "type has no derivable init value and no declared `= value`";

        /// Array without explicit bounds (typl §16.3).
        TYPL_201 = "TYPL-201", Error,
            "array without explicit bounds";

        /// Map without explicit bounds (typl §16.3).
        TYPL_202 = "TYPL-202", Error,
            "map without explicit bounds";

        /// Enum values not unique / not explicitly assigned (typl §16.3).
        TYPL_203 = "TYPL-203", Error,
            "enum values not unique or not explicitly assigned";

        /// Union arm with a primitive type (typl §16.3).
        TYPL_204 = "TYPL-204", Error,
            "union arm with a primitive type";

        /// Recursive composite reference, direct or transitive (typl §16.3).
        TYPL_206 = "TYPL-206", Error,
            "recursive composite reference, direct or transitive";

        /// Enumset bit positions not unique (typl §16.3).
        TYPL_207 = "TYPL-207", Error,
            "enumset bit positions not unique";

        /// `string`/`bytes` used directly as a field type (typl §16.3).
        TYPL_208 = "TYPL-208", Error,
            "`string`/`bytes` used directly as a field type";

        /// Map key is not a named string type or a primitive (typl §16.3).
        TYPL_209 = "TYPL-209", Error,
            "map key is not a named string type or a primitive";

        /// Field, arm, or enum value re-declared under a `reserved` name or value
        /// (typl §16.3).
        TYPL_210 = "TYPL-210", Error,
            "field, arm, or enum value re-declared under a `reserved` entry";

        /// Duplicate `reserved` entry (typl §16.3). Warning. The "dangling"
        /// half of the §16.3 rule (a name/value never previously used) needs the
        /// previous IR snapshot and belongs to `ridl-diff` (E2.8).
        TYPL_211 = "TYPL-211", Warning,
            "duplicate `reserved` entry";

        /// `error` modifier on a declaration other than `enum`, `struct`, `union`
        /// (typl §16.3).
        TYPL_212 = "TYPL-212", Error,
            "`error` modifier on a declaration other than `enum`, `struct`, `union`";

        /// Union mixing error and non-error arms without the result-union shape
        /// (typl §16.3).
        TYPL_213 = "TYPL-213", Error,
            "union mixes error and non-error arms without the result-union shape";

        /// `error union` containing a non-error-typed arm (typl §16.3).
        TYPL_214 = "TYPL-214", Error,
            "`error union` contains a non-error-typed arm";

        /// Stream type `<T>` outside interaction position (typl §16.4, ridl
        /// §12.3). Emitted by the parser in a `.typl` parse (E2 task 2) and by
        /// the checker for struct fields and collections in a `.ridl` file
        /// (E2 task 5).
        TYPL_301 = "TYPL-301", Error,
            "stream type `<T>` outside interaction position";

        /// Timing annotation or duration literal in a typl context (typl §16.4).
        TYPL_302 = "TYPL-302", Error,
            "timing annotation or duration literal in a typl context";

        /// `require`/`ensure` attribute in a typl context (typl §16.4): the two
        /// contract attributes at declaration-start position in a `.typl` parse.
        /// Emitted by the parser (E2 task 2) as a bare string literal rather than
        /// through this constant — see `codes_written_as_string_literals_are_all
        /// _catalogued`.
        TYPL_303 = "TYPL-303", Error,
            "`require`/`ensure` attribute in a typl context";

        /// Interaction declaration in a typl context (typl §16.4, ADR-0007
        /// decision 10): one of the nine ridl words at declaration-start position
        /// in a `.typl` parse. Emitted by the parser (E2 task 2).
        TYPL_304 = "TYPL-304", Error,
            "interaction declaration in a typl context";

        /// Blank line between a doc comment and its definition (typl §14, §16.5).
        /// Warning. Emitted by the checker (E1.7b).
        TYPL_404 = "TYPL-404", Warning,
            "blank line between a doc comment and its definition";

        /// `@deprecated` doc tag without a reason string (typl §14.2, §16.5).
        /// Warning. Emitted by the checker (E1.7b).
        TYPL_405 = "TYPL-405", Warning,
            "`@deprecated` doc tag without a reason string";
    }

    /// The ridl catalogue (ADR-0008 decision 21): every `RIDL-` code declared in
    /// this module, with the severity the ridl reference §16 tables classify it
    /// at. RIDL-140, RIDL-141, and RIDL-143 sit in the 1xx band while the
    /// reference lists them under the §16.4 evolution table — a documented
    /// anomaly kept as written (ADR-0008 decision 6). RIDL-111 and RIDL-142 are
    /// reserved by ADR-0008 decision 21 and are not declared yet, so they are
    /// absent here too.
    ///
    /// Adding a code here does **not** make it show up in a corpus fixture.
    /// `RIDL_PROFILE_CODES` in `crates/ridlc/tests/corpus.rs` — the list that
    /// gives every ridl code a living example — is a list of code *strings*
    /// with no link to these constants, so a code minted here and omitted there
    /// compiles and passes the suite. Decision 21 asks the declare-once
    /// mechanism to cover that list as well, and it does not: the list carries a
    /// `Provoked` discriminator this catalogue has no equivalent of. Stated
    /// here as decision 21 requires; the gap is separate work, tracked in issue
    /// #172.
    RIDL_CATALOG {
        /// `signal` or `event` without a timing annotation — the default
        /// `[100ms..1000ms]` (or the configured `[defaults].timing`) is applied
        /// (ridl §9.1, §16.1). Warning. Emitted by the checker (E2 task 9).
        RIDL_100 = "RIDL-100", Warning,
            "`signal` or `event` without a timing annotation";

        /// A range annotation `@[X..Y]` whose lower bound exceeds its upper bound
        /// (ridl §9.2, §16.1). Emitted by the checker (E2 task 9).
        RIDL_101 = "RIDL-101", Error,
            "timing range `@[X..Y]` with `X > Y`";

        /// A zero or negative timing duration (ridl §9.2, §16.1). Emitted by the
        /// checker (E2 task 9).
        RIDL_102 = "RIDL-102", Error,
            "zero or negative timing duration";

        /// A strict-periodic `@Xms` annotation on an `event` — strict periodic is
        /// signal only (ridl §9.2, §16.1). Emitted by the checker (E2 task 9).
        RIDL_103 = "RIDL-103", Error,
            "strict-periodic `@Xms` on an `event`";

        /// Explicit return type on a `command` — a command always returns `()`
        /// (ridl §6.1, §16.1). Emitted by the checker (E2 task 5).
        RIDL_104 = "RIDL-104", Error,
            "explicit return type on a `command`";

        /// `query` returning `()` — use `command` (ridl §7.1, §16.1). Emitted by
        /// the checker (E2 task 5).
        RIDL_105 = "RIDL-105", Error,
            "`query` returning `()`";

        /// A timing annotation on an interaction kind that carries none —
        /// `command`, `query`, or `final` — or an attribute block on `final`
        /// (ridl §8, §9, §16.1). Emitted by the checker (E2 task 5).
        ///
        /// The callables drew FORM-102 until the E2 close-out, so one rule sat
        /// under two codes and one of them was a parse code whose catalogue
        /// meaning is "unexpected token" — for a token the grammar accepts on
        /// purpose, precisely so the narrowing can be a semantic rule with a
        /// semantic message.
        RIDL_106 = "RIDL-106", Error,
            "timing annotation on a kind that carries none, or attribute block on `final`";

        /// Type declaration inside an `interface` or `service` body — typl
        /// declarations live at package level (ridl §14.1, §16.1).
        ///
        /// Emitted by the **parser**, as a bare string literal (`ridl-syntax`
        /// cannot reference [`DiagCode`]), at the point where it recognises the
        /// keyword and recovers the declaration into an `ErrorNode`. The
        /// checker used to code that node a second time, so every RIDL-107
        /// arrived paired with a contradicting FORM-102 at the same span; the
        /// parser knows exactly what the construct is, so it is the one that
        /// names it. RIDL-403 and TYPL-304 are parser-raised for the same
        /// reason. The constant is kept for the catalogue and for the error
        /// index.
        RIDL_107 = "RIDL-107", Error,
            "type declaration inside an `interface` or `service` body";

        /// A range annotation `@[X..X]` whose bounds are equal — a degenerate
        /// range, the rate floor equal to its staleness bound, on a `signal` and an
        /// `event` alike (ridl §9.2, §16.1; ADR-0008 decision 17). Not a spelling
        /// of the strict-periodic `@Xms`, which is a separate `TimingMode`.
        /// Warning. Emitted by the checker (E2 task 9).
        RIDL_108 = "RIDL-108", Warning,
            "degenerate timing range `@[X..X]`";

        /// Signal payload type has no derivable init value and no `= value`
        /// override (ridl §4.4, §16.1). Emitted by the checker (E2 task 5).
        RIDL_109 = "RIDL-109", Error,
            "signal payload has no derivable init and no `= value` override";

        /// Signal `= value` init override violates the payload type's constraints
        /// (ridl §4.4, §16.1). Emitted by the checker (E2 task 5).
        RIDL_110 = "RIDL-110", Error,
            "signal `= value` init override violates the payload constraints";

        /// Duplicate `service` name across the whole workspace — the service
        /// catalog is a flat global namespace (ridl §14.5, §16.4). Emitted
        /// workspace-wide by `service_catalog` (E2 task 8). The reference numbers
        /// it in the 1xx band while listing it under the §16.4 evolution/profile
        /// table — a documented anomaly kept as written (ADR-0008 decision 6).
        RIDL_140 = "RIDL-140", Error,
            "duplicate `service` name across the workspace";

        /// A `service` names a type that is not an `interface`, and has no inline
        /// shape (ridl §14.5, §16.4). Emitted per-package by the checker (E2 task
        /// 8). Kept in the 1xx band per ADR-0008 decision 6 (see RIDL-140).
        RIDL_141 = "RIDL-141", Error,
            "`service` names a type that is not an `interface`";

        /// A `service` publishes an `internal` interface (ridl §14.5, §16.4).
        /// Emitted per-package by the checker's exposure pass. Distinct from
        /// TYPL-005: what leaks is an interface rather than a type, and a service
        /// takes no `internal` modifier, so the TYPL-005 remedy — make the
        /// exposing declaration internal too — does not exist here. Kept in the
        /// 1xx band beside RIDL-140/-141 per ADR-0008 decision 6. RIDL-111 and
        /// RIDL-142 are reserved by decision 21 and not yet implemented, so 143 is
        /// the next free code; decision 13's allocation ledger needs the ninth
        /// entry (issue #169).
        RIDL_143 = "RIDL-143", Error,
            "`service` publishes an `internal` interface";

        /// Stream `<T>` on a `signal` or `event` payload (ridl §12.3, §16.2).
        /// Emitted by the checker (E2 task 5).
        RIDL_201 = "RIDL-201", Error,
            "stream `<T>` on a `signal` or `event` payload";

        /// Stream element type not a named type, `string`, or `bytes` (ridl
        /// §12.2, §16.2). Emitted by the checker (E2 task 5).
        RIDL_202 = "RIDL-202", Error,
            "stream element type not a named type, `string`, or `bytes`";

        /// `require` or `ensure` on `signal`, `event`, or `final` (ridl §13,
        /// §16.3). Emitted by the checker (E2 task 5).
        RIDL_301 = "RIDL-301", Error,
            "`require` or `ensure` on `signal`, `event`, or `final`";

        /// `ensure` on `command` — a command has no result to observe (ridl §6.1,
        /// §16.3). Emitted by the checker (E2 task 5).
        RIDL_302 = "RIDL-302", Error,
            "`ensure` on `command`";

        /// A fallible query return with no success path (ridl §10.1, §16.3; general
        /// form §6.1): a bare `error` type in return position, an `error`-typed
        /// success (left) arm of an inline `T | E`, or a non-error error (right)
        /// arm. Error. Emitted by the checker (E2 task 10).
        RIDL_303 = "RIDL-303", Error,
            "fallible query return with no success path";

        /// An `error`-typed or result-union parameter on a `command` or `query` —
        /// failure flowing toward a provider (ridl §10.1, §16.3). Warning. Emitted
        /// by the checker (E2 task 10).
        RIDL_304 = "RIDL-304", Warning,
            "`error`-typed or result-union parameter on a `command` or `query`";

        /// An `ensure` clause that never references `result` — well-typed but
        /// suspicious (ridl §13, §16.3; expr-core specification §8). Warning.
        /// Emitted by the checker (E2 task 11).
        RIDL_305 = "RIDL-305", Warning,
            "`ensure` clause that never references `result`";

        /// A `require`/`ensure` expression outside the guaranteed subset (ridl §13,
        /// §16.3; expr-core specification §8 — one code for the whole boundary,
        /// with a message naming the offending form). Error. Emitted by the checker
        /// (E2 task 11).
        RIDL_306 = "RIDL-306", Error,
            "`require`/`ensure` expression outside the guaranteed subset";

        /// An `error` enum declares a Stratum-2 contract-error category name
        /// (`INVALID_VALUE`, `PRECONDITION_FAILED`, `CONTRACT_BROKEN`,
        /// `UNKNOWN_INTERACTION`) — reserved vocabulary (ridl §10.2, §16.3).
        /// Warning. Emitted by the checker (E2 task 10).
        RIDL_307 = "RIDL-307", Warning,
            "contract-error category name declared in an `error` enum";

        /// A named result union in query return position — the inline `T | E`
        /// spelling is canonical there (general form §6.1, ADR-0008 decision 13).
        /// Warning; the named spelling stays legal typl data, so this is a lint,
        /// not an error. Emitted by the lint pass (E2 task 19).
        RIDL_308 = "RIDL-308", Warning,
            "named result union in query return position";

        /// Interaction re-declared under a `reserved` name (ridl §11, §16.4).
        /// Emitted by the checker (E2 task 5).
        RIDL_401 = "RIDL-401", Error,
            "interaction re-declared under a `reserved` name";

        /// Duplicate interaction name within an interface (ridl §14.1, §16.4).
        /// Emitted by the checker (E2 task 5); lowering keeps the first
        /// declaration only.
        RIDL_402 = "RIDL-402", Error,
            "duplicate interaction name within an interface";

        /// Behaviour, user-interaction, or architecture declaration in a ridl
        /// context (ridl §16.4): a reserved word of the uxdl/rmdl/rsdl profiles at
        /// declaration-start position in a `.ridl` parse. Emitted by the parser
        /// (E2 task 2).
        RIDL_403 = "RIDL-403", Error,
            "behaviour, user-interaction, or architecture declaration in a ridl context";

        /// A query named like a mutation — `set…`, `reset…`, and the rest of the
        /// mutating verb set (ridl §7.2, §16.4): a state-mutating request belongs
        /// to `command`. Warning. Emitted by the lint pass (E2 task 19).
        RIDL_404 = "RIDL-404", Warning,
            "query named like a mutation";

        /// One `error` type used as the failure arm of queries in three or more
        /// distinct interfaces — the "shared across unrelated failure domains"
        /// heuristic (ridl §10.1, §16.4). Info. Emitted by the lint pass (E2 task
        /// 19); the threshold is three, so two interfaces stay silent.
        RIDL_405 = "RIDL-405", Info,
            "one `error` type shared across unrelated failure domains";

        /// A `signal` or `event` payload whose struct re-declares envelope
        /// metadata — publication time or a frame counter (ridl §3.1, §16.4). Info;
        /// domain time distinct from transport time is legitimate, so the message
        /// says so. Emitted by the lint pass (E2 task 19).
        RIDL_406 = "RIDL-406", Info,
            "payload struct re-declares envelope metadata";

        /// An interaction ordinal changed against a published baseline snapshot
        /// (ridl §11, general form §6.3). Warning. Emitted by the `ridl check` desk
        /// check (E2 task 18), never by the compiler: the comparison reads a
        /// workspace-local baseline, which is outside `ridlc`'s source→IR function
        /// (ADR-0008 decisions 9 and 13).
        RIDL_407 = "RIDL-407", Warning,
            "interaction ordinal changed against the published baseline";
    }

    /// The manifest catalogue (ADR-0007 decision 2): the manifest `0xx` codes the
    /// `ridl.toml` parser (E1.5) and the package loader (E1.3) emit, and the
    /// distribution `1xx` codes the import materializer (E1.6) emits. Listed here
    /// even for `MANI-004`, whose emission site is the loader rather than the
    /// standalone parser, so the error index (E4.2) has one authoritative source.
    MANI_CATALOG {
        /// The `ridl.toml` text is not valid TOML.
        MANI_001 = "MANI-001", Error,
            "invalid manifest TOML";

        /// The manifest declares both `[package]` and `[workspace]`; the two modes
        /// are mutually exclusive (ADR-0002 §4).
        MANI_002 = "MANI-002", Error,
            "manifest declares both `[package]` and `[workspace]`";

        /// The manifest declares neither `[package]` nor `[workspace]` (ADR-0002 §4).
        MANI_003 = "MANI-003", Error,
            "manifest declares neither `[package]` nor `[workspace]`";

        /// A workspace member's own manifest declares `[workspace]`; nested
        /// workspaces are forbidden (ADR-0002 §4). Defined here, but emitted by the
        /// package loader (E1.3, task 8) when a member manifest is read — a single
        /// manifest parsed in isolation cannot know it is a member.
        MANI_004 = "MANI-004", Error,
            "nested workspace: a member manifest declares `[workspace]`";

        /// An unrecognized key in the manifest or one of its sections (warning).
        MANI_005 = "MANI-005", Warning,
            "unknown manifest key";

        /// The package name is not lowercase dot-separated segments (ADR-0002 §1).
        MANI_006 = "MANI-006", Error,
            "invalid package name";

        /// An `[imports]` value is not a valid import URL.
        MANI_007 = "MANI-007", Error,
            "invalid import URL";

        /// A workspace member directory is missing or has no `ridl.toml`. Emitted
        /// by the package loader (E1.3), which is where member paths are resolved
        /// against the filesystem.
        MANI_008 = "MANI-008", Error,
            "workspace member directory has no `ridl.toml`";

        /// The manifest `[defaults].timing` value is not a valid range (ridl §9.1,
        /// ADR-0008 decision 13). The manifest parser stores the raw string
        /// unparsed — `ridl-core` cannot depend on `ridl-sem` — so the checker
        /// parses it and emits this code (E2 task 9).
        MANI_009 = "MANI-009", Error,
            "invalid `[defaults].timing` value";

        /// A remote import could not be fetched (network failure, a non-2xx HTTP
        /// status, or a value that is not a fetchable `http(s)` URL).
        MANI_101 = "MANI-101", Error,
            "remote import fetch failed";

        /// Fetched content hashes to a value that does not match the SHA-256 the
        /// lockfile pins for the same URL (ADR-0002 §7).
        MANI_102 = "MANI-102", Error,
            "fetched content hash does not match the lockfile";

        /// `--frozen` was requested but the lockfile has no entry for a remote
        /// import; a frozen build never regenerates the lockfile (ADR-0002 §7).
        MANI_103 = "MANI-103", Error,
            "`--frozen`: no lockfile entry for a remote import";

        /// `--frozen` was requested and a lockfile-pinned import is not present in
        /// the cache; a frozen build never fetches (ADR-0002 §7).
        MANI_104 = "MANI-104", Error,
            "`--frozen`: a lockfile-pinned import is not cached";
    }
}

/// A diagnostic's severity. Warnings and info diagnostics arrive with later
/// passes; every code the E1.10 pipeline emits is an [`Error`](Severity::Error).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Severity {
    Error,
    Warning,
    Info,
}

/// An interned file id, issued by [`SourceMap::file_id`]. It indexes the file's
/// path and text inside the [`SourceMap`], which the renderer reads to draw the
/// source snippet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct FileId(u32);

impl FileId {
    /// A sentinel id for a diagnostic that is not tied to any source file. The
    /// lockfile, cache, and fetch diagnostics (MANI-1xx) concern a URL rather
    /// than a byte span, so they carry this id; [`render`](render()) draws them as a bare
    /// coded message with no source snippet. No [`SourceMap`] ever issues it.
    pub const DETACHED: FileId = FileId(u32::MAX);
}

/// A source location: a byte range inside a specific file. The range is a
/// `rowan::TextRange` — the same coordinate space parse and semantic passes work
/// in — so an offset never has to be translated on the way into a diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Span {
    pub file: FileId,
    #[serde(serialize_with = "serialize_text_range")]
    pub range: TextRange,
}

/// Serializes a `rowan::TextRange` as `{ "start": u32, "end": u32 }`, keeping
/// the byte offsets readable and exact in JSON snapshots.
fn serialize_text_range<S: Serializer>(
    range: &TextRange,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    use serde::ser::SerializeStruct;
    let mut state = serializer.serialize_struct("TextRange", 2)?;
    state.serialize_field("start", &u32::from(range.start()))?;
    state.serialize_field("end", &u32::from(range.end()))?;
    state.end()
}

/// A secondary annotation: a span with a message, drawn under the source
/// alongside the primary span.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Label {
    pub span: Span,
    pub message: String,
}

/// A suggested edit: replace the text at `span` with `replacement`. `label`
/// describes the fix for a human. Rendered as a note under the diagnostic; a
/// later LSP task maps it to a code action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FixIt {
    pub span: Span,
    pub replacement: String,
    pub label: String,
}

/// One coded diagnostic. `primary` is the main span the message points at;
/// `labels` are secondary annotations; `fixits` are suggested edits.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Diagnostic {
    pub code: DiagCode,
    pub severity: Severity,
    pub message: String,
    pub primary: Span,
    pub labels: Vec<Label>,
    pub fixits: Vec<FixIt>,
}

/// The path and text of one interned file.
#[derive(Debug)]
struct SourceEntry {
    path: String,
    text: String,
}

/// The file table the renderer reads: it maps every [`FileId`] to the path and
/// text a diagnostic's spans point into. Ids are interned by path, so asking for
/// the same path twice returns the same id.
#[derive(Debug, Default)]
pub struct SourceMap {
    files: Vec<SourceEntry>,
}

impl SourceMap {
    /// An empty source map.
    pub fn new() -> Self {
        Self::default()
    }

    /// The [`FileId`] for `path`, interning `path` and `text` on first sight.
    /// A pass holding a file's path and text calls this to stamp its spans.
    pub fn file_id(&mut self, path: &str, text: &str) -> FileId {
        if let Some(index) = self.files.iter().position(|entry| entry.path == path) {
            return FileId(index as u32);
        }
        let id = FileId(self.files.len() as u32);
        self.files.push(SourceEntry {
            path: path.to_string(),
            text: text.to_string(),
        });
        id
    }

    /// The path an interned [`FileId`] stands for, or `None` for an id this map
    /// never issued (including [`FileId::DETACHED`]). This is the reverse of
    /// [`file_id`](Self::file_id): a caller holding a diagnostic's `FileId` reads
    /// back the file it points at without probing candidate paths.
    pub fn path(&self, id: FileId) -> Option<&str> {
        self.files
            .get(id.0 as usize)
            .map(|entry| entry.path.as_str())
    }
}

/// Remaps per-package diagnostics onto a renderer's [`SourceMap`] ids.
///
/// The package-scoped passes (`resolve_package`, `check_package` in
/// `ridl-sem`) stamp their spans with a [`FileId`] that indexes the package's
/// `files` **in order** — they run inside salsa queries and cannot share the
/// caller's [`SourceMap`]. A renderer first interns each package file into its
/// own map (collecting the issued ids in the same file order), then calls this
/// to rewrite every span onto those ids. [`FileId::DETACHED`] spans and any
/// index past `render_ids` are left untouched.
pub fn remap_diagnostics(
    diagnostics: impl IntoIterator<Item = Diagnostic>,
    render_ids: &[FileId],
) -> Vec<Diagnostic> {
    let remap_file =
        |file: FileId| -> FileId { render_ids.get(file.0 as usize).copied().unwrap_or(file) };
    diagnostics
        .into_iter()
        .map(|mut diagnostic| {
            diagnostic.primary.file = remap_file(diagnostic.primary.file);
            for label in &mut diagnostic.labels {
                label.span.file = remap_file(label.span.file);
            }
            for fixit in &mut diagnostic.fixits {
                fixit.span.file = remap_file(fixit.span.file);
            }
            diagnostic
        })
        .collect()
}

/// One row of a diagnostic catalogue: a code, its default severity, and a short
/// human summary. The catalogue is the static SSOT the error index (E4.2) reads;
/// the per-diagnostic [`Severity`] a pass emits is set independently.
#[derive(Debug, Clone, Copy)]
pub struct CatalogEntry {
    pub code: DiagCode,
    pub severity: Severity,
    pub summary: &'static str,
}

/// Polishes a raw parser message into the house diagnostic style —
/// description-first, with backticked names (fixes issue #102). Most parser
/// messages are already house-style and pass through unchanged; the parser's
/// `expect` path emits a `Debug`-shaped token name (`expected RBracket`), which
/// this maps to a backticked glyph (`` expected `]` ``).
///
/// Keeping the raw parser message as the input leaves `ridl-syntax` and its
/// tests untouched — they assert on codes and ranges, not message text — while
/// the rendered diagnostics still read in one consistent style.
pub fn house_style_message(raw: &str) -> String {
    if let Some(token) = raw.strip_prefix("expected ")
        && let Some(glyph) = punctuation_glyph(token)
    {
        return format!("expected {glyph}");
    }
    raw.to_string()
}

/// The backticked glyph for a punctuation `SyntaxKind` `Debug` name, or `None`
/// when the name is not a known punctuation token. Covers every punctuation kind
/// the parser can name in a FORM-101 `expected` message, so the mapping stays
/// correct if new `expect` call sites are added.
fn punctuation_glyph(debug_name: &str) -> Option<&'static str> {
    Some(match debug_name {
        "Colon" => "`:`",
        "Eq" => "`=`",
        "Semicolon" => "`;`",
        "Comma" => "`,`",
        "LBracket" => "`[`",
        "RBracket" => "`]`",
        "LBrace" => "`{`",
        "RBrace" => "`}`",
        "LParen" => "`(`",
        "RParen" => "`)`",
        "DotDot" => "`..`",
        "Dot" => "`.`",
        "Question" => "`?`",
        "At" => "`@`",
        "Pipe" => "`|`",
        _ => return None,
    })
}

impl SourceMap {
    /// The interned files, in id order — the renderer replays them into a
    /// `codespan_reporting` file table so `FileId(i)` lines up with codespan id
    /// `i`.
    pub(crate) fn iter_files(&self) -> impl Iterator<Item = (&str, &str)> {
        self.files
            .iter()
            .map(|entry| (entry.path.as_str(), entry.text.as_str()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_map_interns_by_path() {
        let mut map = SourceMap::new();
        let a = map.file_id("a.typl", "type A: m");
        let b = map.file_id("b.typl", "type B: s");
        let a_again = map.file_id("a.typl", "type A: m");
        assert_ne!(a, b, "distinct paths get distinct ids");
        assert_eq!(a, a_again, "the same path interns to the same id");
    }

    #[test]
    fn source_map_path_reverses_file_id() {
        let mut map = SourceMap::new();
        let a = map.file_id("a.typl", "type A: m");
        let b = map.file_id("b.typl", "type B: s");
        assert_eq!(map.path(a), Some("a.typl"));
        assert_eq!(map.path(b), Some("b.typl"));
        assert_eq!(
            map.path(FileId::DETACHED),
            None,
            "a detached id has no path"
        );
        assert_eq!(
            map.path(FileId(2)),
            None,
            "an id the map never issued has no path",
        );
    }

    #[test]
    fn remap_diagnostics_rewrites_package_relative_ids() {
        // A render map that already interned an unrelated file, so the render
        // ids do not coincide with the package-relative indices.
        let mut render = SourceMap::new();
        let _other = render.file_id("other.typl", "");
        let a = render.file_id("pkg/a.typl", "type A: m");
        let b = render.file_id("pkg/b.typl", "type B: s");
        let render_ids = vec![a, b];

        let span = |file: FileId| Span {
            file,
            range: TextRange::default(),
        };
        let diagnostic = |file: FileId| Diagnostic {
            code: DiagCode::TYPL_009,
            severity: Severity::Error,
            message: "duplicate declaration of `X`".to_string(),
            primary: span(file),
            labels: vec![Label {
                span: span(file),
                message: "first declared here".to_string(),
            }],
            fixits: Vec::new(),
        };

        // Package-relative ids 0 and 1, plus a detached diagnostic.
        let mut pass_map = SourceMap::new();
        let pkg_a = pass_map.file_id("pkg/a.typl", "type A: m");
        let pkg_b = pass_map.file_id("pkg/b.typl", "type B: s");
        let remapped = remap_diagnostics(
            vec![
                diagnostic(pkg_a),
                diagnostic(pkg_b),
                diagnostic(FileId::DETACHED),
            ],
            &render_ids,
        );

        assert_eq!(remapped[0].primary.file, a);
        assert_eq!(remapped[0].labels[0].span.file, a);
        assert_eq!(remapped[1].primary.file, b);
        assert_eq!(
            remapped[2].primary.file,
            FileId::DETACHED,
            "a detached diagnostic stays detached",
        );
    }

    #[test]
    fn house_style_rewrites_debug_token_names() {
        assert_eq!(house_style_message("expected RBracket"), "expected `]`");
        assert_eq!(house_style_message("expected Colon"), "expected `:`");
        assert_eq!(house_style_message("expected Eq"), "expected `=`");
        assert_eq!(house_style_message("expected Semicolon"), "expected `;`");
    }

    #[test]
    fn house_style_passes_through_already_styled_messages() {
        // Already description-first with backticks — unchanged.
        assert_eq!(house_style_message("expected a name"), "expected a name");
        assert_eq!(house_style_message("unclosed `{`"), "unclosed `{`");
        assert_eq!(
            house_style_message("missing `package` declaration"),
            "missing `package` declaration",
        );
    }

    /// Every catalogue entry is well formed, every catalogue is sorted, and no
    /// code is declared twice.
    ///
    /// This is what is left to check once [`diag_codes!`] produces the
    /// catalogues. Completeness is structural — the constant and its entry come
    /// out of the same line — so the assertions here cover only the properties
    /// the expansion does not fix. There is deliberately no expected list of
    /// codes: comparing a catalogue against a second hand-written list is the
    /// guard this change removed.
    #[test]
    fn catalog_entries_are_well_formed_ordered_and_unique() {
        let mut seen: Vec<&str> = Vec::new();
        for (name, catalog) in ALL_CATALOGS {
            let prefix = name
                .strip_suffix("_CATALOG")
                .unwrap_or_else(|| panic!("`{name}` is not named `<PREFIX>_CATALOG`"));
            let mut previous = "";
            for entry in *catalog {
                let code = entry.code.as_str();
                let (written_prefix, number) = code
                    .split_once('-')
                    .unwrap_or_else(|| panic!("`{code}` is not spelled `PREFIX-NNN`"));
                assert_eq!(
                    written_prefix, prefix,
                    "`{code}` is listed in {name}, which holds the `{prefix}-` namespace",
                );
                assert!(
                    number.len() == 3 && number.bytes().all(|byte| byte.is_ascii_digit()),
                    "`{code}` does not carry a three-digit number",
                );
                assert!(!entry.summary.is_empty(), "`{code}` has an empty summary");
                // Codes in one catalogue share a prefix and a three-digit
                // number, so byte order is numeric order.
                assert!(
                    previous < code,
                    "{name} is out of order: `{previous}` is listed before `{code}`",
                );
                previous = code;
                assert!(
                    !seen.contains(&code),
                    "`{code}` is declared in more than one catalogue",
                );
                seen.push(code);
            }
        }
        // Width. The loops above say nothing if `ALL_CATALOGS` is ever empty.
        // The macro cannot produce an empty one, but a floor makes a vacuous
        // pass unreachable rather than merely unlikely, and codes are never
        // withdrawn (ADR-0007 decision 2), so the bound only gets safer.
        assert!(
            seen.len() >= 90,
            "only {} codes reached the catalogues — the guards below are \
             checking almost nothing",
            seen.len(),
        );
    }

    /// Each constant's name is the code string it expands to, with `-` written
    /// `_`.
    ///
    /// [`diag_codes!`] takes the two side by side — `TYPL_007 = "TYPL-007"` — so
    /// a typo can still pair a name with another code's string, and every
    /// emission through that constant would then render the wrong code.
    /// `CODE_CONSTANT_NAMES` comes out of the same entries, so this compares the
    /// expansion against itself and not against a list someone maintains.
    #[test]
    fn each_constant_name_is_the_code_it_expands_to() {
        for (name, code) in CODE_CONSTANT_NAMES {
            assert_eq!(
                &name.replace('_', "-"),
                code,
                "`DiagCode::{name}` expands to `{code}`",
            );
        }
        assert!(CODE_CONSTANT_NAMES.len() >= 90, "the entry list went empty");
    }

    /// The two namespace-wide severity rules: every FORM code is an error, and
    /// every MANI code is an error but the unknown-key warning.
    ///
    /// Neither rule enumerates codes, so neither is a shadow list — one states a
    /// property of a whole namespace, the other adds a single named exception.
    /// TYPL and RIDL have no such rule: their severities are per-code, set by
    /// the reference §16 tables, and writing them out here would rebuild exactly
    /// the second list this change removed. A wrong severity on a TYPL or RIDL
    /// entry is not caught by anything in this module.
    #[test]
    fn form_and_mani_severities_follow_their_namespace_rule() {
        for entry in FORM_CATALOG {
            assert_eq!(
                entry.severity,
                Severity::Error,
                "every FORM code is an error, but {} is not",
                entry.code.as_str(),
            );
        }
        for entry in MANI_CATALOG {
            let expected = if entry.code == DiagCode::MANI_005 {
                Severity::Warning
            } else {
                Severity::Error
            };
            assert_eq!(
                entry.severity,
                expected,
                "unexpected severity for {}",
                entry.code.as_str(),
            );
        }
    }

    /// Every `"PREFIX-NNN"` string literal in the workspace's Rust sources names
    /// a catalogued code.
    ///
    /// This is the half [`diag_codes!`] cannot reach, and it is not a handful of
    /// call sites that forgot to use the type — it is a layering fact.
    /// `crates/ridl-syntax` cannot reference [`DiagCode`] at all: `ridl-core`
    /// depends on `ridl-syntax`, so the edge cannot run the other way, and
    /// `SyntaxError::code` is a `&'static str` by construction. Every code the
    /// lexer and parser emit is therefore a bare string literal — 11 distinct
    /// codes across 74 call sites when this guard was written. Five of them have
    /// no `DiagCode::` reference anywhere and their constants are dead
    /// declarations (FORM-005, FORM-104, FORM-105, RIDL-403, TYPL-304); TYPL-303
    /// had no constant at all until this change added one. Repairing that means
    /// moving the codes somewhere both crates can see, which is its own change
    /// (issue #172).
    ///
    /// **What this catches.** A code emitted, asserted, or declared anywhere in
    /// the workspace's `.rs` files that no catalogue lists — a new parser
    /// diagnostic included, verified by renaming the parser's TYPL-303 emission
    /// and watching this fail and name the file.
    ///
    /// **What this does not catch**, and must not be read as covering:
    ///
    /// - a code assembled rather than spelled: `concat!("TYPL-", "303")`, or one
    ///   built at run time. The scan is textual. `diag_codes!` takes a `literal`
    ///   so the assembled form cannot be written inside it, and
    ///   `no_diagnostic_constant_is_declared_outside_the_macro` covers the
    ///   hand-written-constant case; a `concat!` at a *parser* emission site
    ///   evades both;
    /// - a code spelled **inside a longer literal**, such as
    ///   `"error[TYPL-905]: boom"`. A quote is required on both sides of the
    ///   code, which is what keeps prose out, and it is also what lets an
    ///   embedded code through;
    /// - a **four-digit** code, `"TYPL-9060"`. The scan takes exactly three
    ///   digits followed by the closing quote, matching the shape ADR-0007
    ///   decision 2 fixes. Neither of these two is live: an audit of all 104
    ///   `PREFIX-NNN` occurrences in `.rs` sources regardless of quoting found
    ///   the only uncatalogued ones are the reserved codes and the typl §16
    ///   codes named below, every one of them in prose;
    /// - a code written in a **comment**, which is stripped before the scan
    ///   runs. Nothing emits a diagnostic from a comment, and leaving comments
    ///   in made this file report its own prose about reserved and absent
    ///   codes;
    /// - a code written only in Markdown, in a `.typl`/`.ridl` fixture, or in a
    ///   snapshot. The typl reference §16 documents six codes no constant
    ///   declares — TYPL-107, TYPL-112, TYPL-205, and TYPL-401 to TYPL-403 — so
    ///   widening the scan to `.md` would fail today. That inventory belongs to
    ///   issue #172, not to this guard;
    /// - a catalogued code that nothing emits. FORM-001 to FORM-004 are declared
    ///   and catalogued and no pass emits them;
    /// - a code **withdrawn** from a catalogue. Deleting an entry deletes its
    ///   constant, so any code a pass emits through `DiagCode::` stops the build
    ///   — but a code nothing references, such as FORM-001, can be removed and
    ///   nothing here notices. The floor in
    ///   `catalog_entries_are_well_formed_ordered_and_unique` catches a bulk
    ///   withdrawal, not a single one;
    /// - a wrong severity, or a summary that describes the wrong rule, on an
    ///   entry that exists.
    #[test]
    fn codes_written_as_string_literals_are_all_catalogued() {
        let root = workspace_root();
        let mut sources = Vec::new();
        collect_rust_sources(&root, &mut sources);

        let catalogued: std::collections::BTreeSet<&str> = ALL_CATALOGS
            .iter()
            .flat_map(|(_, catalog)| catalog.iter().map(|entry| entry.code.as_str()))
            .collect();

        let mut uncatalogued: Vec<String> = Vec::new();
        let mut files_holding_codes = 0usize;
        let mut codes_outside_this_module: std::collections::BTreeSet<String> =
            std::collections::BTreeSet::new();

        for path in &sources {
            let text = std::fs::read_to_string(path)
                .unwrap_or_else(|err| panic!("cannot read {}: {err}", path.display()));
            // Comments first: a diagnostic is never emitted from one, and prose
            // about a code that is deliberately absent — a reserved number, a
            // worked example of the shape this file rejects — would otherwise
            // be reported as an escape.
            let text = strip_line_comments(&text);
            let literals = code_literals(&text);
            if !literals.is_empty() {
                files_holding_codes += 1;
            }
            let is_this_module = path.ends_with("ridl-core/src/diag.rs");
            for code in literals {
                if !catalogued.contains(code) {
                    uncatalogued.push(format!("{}: {code}", path.display()));
                }
                if !is_this_module {
                    codes_outside_this_module.insert(code.to_string());
                }
            }
        }

        assert!(
            uncatalogued.is_empty(),
            "these code strings are written in Rust sources but no catalogue \
             lists them, so the error index (E4.2) has nothing to key them on \
             and nothing connects them to a `DiagCode`. Declare each one in \
             `diag_codes!`:\n{}",
            uncatalogued.join("\n"),
        );

        // Width. Every assertion above is vacuous if the walk finds nothing, and
        // a walk rooted at the wrong directory finds nothing quietly. These
        // floors are well under what the workspace holds today — 79 `.rs` files,
        // 21 of them carrying a code, 92 distinct codes outside this module —
        // and fail loudly if the scan stops reaching past its own crate. The
        // three figures are measured, not maintained: they are a comment, and
        // the assertions below hold whether or not they drift.
        assert!(
            sources.len() >= 60,
            "the walk found only {} `.rs` files under {} — it is not reaching \
             the workspace",
            sources.len(),
            root.display(),
        );
        assert!(
            files_holding_codes >= 10,
            "only {files_holding_codes} files carried a code literal",
        );
        assert!(
            codes_outside_this_module.len() >= 60,
            "only {} distinct codes were seen outside `diag.rs` — the scan is \
             checking this module against itself",
            codes_outside_this_module.len(),
        );
    }

    /// No `DiagCode` constant is declared outside [`diag_codes!`], anywhere in
    /// the workspace.
    ///
    /// The scan above already reports a hand-written constant whose code is
    /// spelled as a literal, because the literal is what it looks for — but one
    /// whose code is assembled compiles and slips past it. This catches that
    /// form, and any other, by looking at the declaration rather than at the
    /// code string.
    ///
    /// The surface is the whole workspace, not this crate. An inherent
    /// `impl DiagCode` can only be written here, but the newtype's field is
    /// public, so any crate can construct one and give it a name. A constant of
    /// this type declared in `ridl-sem` with an assembled code compiles and
    /// passes both the suite and clippy when this walks `ridl-core` alone, so it
    /// walks everything.
    ///
    /// Matching is whitespace-insensitive and accepts a path-qualified type,
    /// because a declaration in another crate writes the type as
    /// `ridl_core::diag::…` and rustfmt may break it across lines. Comments are
    /// stripped first, so prose describing the forbidden shape — including this
    /// paragraph — does not report itself; the cost is that a `//` inside a
    /// string literal hides the rest of that line from the scan.
    ///
    /// It remains a textual check: it recognises the two shapes a `DiagCode`
    /// constant is written in and rejects everything else, so a declaration
    /// reworded to avoid both evades it. The two evasions are not symmetric,
    /// and only their combination gets through:
    ///
    /// - a declaration writing the type under an alias (`use … DiagCode as DC;`
    ///   then `const RIDL_199: DC = DC("RIDL-199");`) is invisible here, but its
    ///   code is spelled, so `codes_written_as_string_literals_are_all_catalogued`
    ///   reports it;
    /// - a declaration with an assembled code is invisible to that scan, but
    ///   names the type, so this one reports it — verified against a qualified,
    ///   line-broken `concat!` declaration in `ridl-sem` and against one in a
    ///   child module of `diag`;
    /// - **both at once** — an aliased type and an assembled code — passes the
    ///   whole workspace suite and clippy. That is a surviving escape, and it is
    ///   a deliberate act rather than the omission this file defends against.
    #[test]
    fn no_diagnostic_constant_is_declared_outside_the_macro() {
        // Assembled rather than spelled: this test lives in a file it scans, so
        // writing the shapes out whole would make the test report itself.
        let anchor = format!("DiagCode{}", '=');
        // The macro body, expanding one entry into its constant.
        let in_macro = format!("pubconst$konst:{anchor}DiagCode($code);");
        // The sentinel, which has no catalogue entry by design.
        let sentinel = format!("pubconstNONE:{anchor}DiagCode(\"\");");
        let accepted = [in_macro.as_str(), sentinel.as_str()];

        let root = workspace_root();
        let mut sources = Vec::new();
        collect_rust_sources(&root, &mut sources);
        assert!(
            sources.len() >= 60,
            "the walk found only {} `.rs` files under {} — it is not reaching \
             the workspace",
            sources.len(),
            root.display(),
        );

        let mut declarations = 0usize;
        for path in &sources {
            let text = std::fs::read_to_string(path)
                .unwrap_or_else(|err| panic!("cannot read {}: {err}", path.display()));
            let stripped = strip_line_comments(&text);
            let despaced: String = stripped.chars().filter(|c| !c.is_whitespace()).collect();

            for (at, _) in despaced.match_indices(&anchor) {
                let recognised = accepted.iter().any(|form| {
                    let offset = form.find(&anchor).expect("each form holds the anchor");
                    at >= offset && despaced[at - offset..].starts_with(form)
                });
                assert!(
                    recognised,
                    "{}: `…{}…` declares a `DiagCode` constant outside \
                     `diag_codes!`, so it carries no catalogue entry. Move it \
                     into the macro.",
                    path.display(),
                    &despaced[at.saturating_sub(40)..(at + 40).min(despaced.len())],
                );
                declarations += 1;
            }
        }
        assert_eq!(
            declarations, 2,
            "expected exactly two `DiagCode` constant declarations in the \
             workspace — the macro body and the `NONE` sentinel, both in this \
             file",
        );
    }

    /// `text` with every `//` line comment removed, so prose about a forbidden
    /// code shape is not mistaken for the shape itself. A `//` inside a string
    /// literal takes the rest of its line with it.
    fn strip_line_comments(text: &str) -> String {
        text.lines()
            .map(|line| match line.find("//") {
                Some(at) => &line[..at],
                None => line,
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// The workspace root: two levels above `crates/ridl-core`.
    fn workspace_root() -> std::path::PathBuf {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(std::path::Path::parent)
            .expect("`crates/ridl-core` sits two levels below the workspace root")
            .to_path_buf();
        let manifest = std::fs::read_to_string(root.join("Cargo.toml"))
            .unwrap_or_else(|err| panic!("no manifest at {}: {err}", root.display()));
        assert!(
            manifest.contains("[workspace]"),
            "{} is not the workspace root",
            root.display(),
        );
        root
    }

    /// Every `.rs` file under `dir`, skipping `target` and dot-directories — the
    /// latter keeps the walk out of `.git` and out of any `.claude/worktrees`
    /// checkout of this same repository.
    fn collect_rust_sources(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        let entries = std::fs::read_dir(dir)
            .unwrap_or_else(|err| panic!("cannot read {}: {err}", dir.display()));
        for entry in entries {
            let entry = entry.unwrap_or_else(|err| panic!("cannot read {}: {err}", dir.display()));
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with('.') || name == "target" {
                continue;
            }
            let path = entry.path();
            if path.is_dir() {
                collect_rust_sources(&path, out);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                out.push(path);
            }
        }
    }

    /// Every `"PREFIX-NNN"` string literal in `text`, where `PREFIX` is two or
    /// more uppercase ASCII letters.
    ///
    /// The quotes are required on both sides, which is what keeps prose out: the
    /// doc comments in this module name RIDL-111 and RIDL-142 as reserved and
    /// not yet declared, and a scan that read unquoted text would report them.
    /// Matching is position-local rather than quote-pairing, so an escaped quote
    /// earlier in a string cannot shift the scan out of alignment.
    fn code_literals(text: &str) -> Vec<&str> {
        let bytes = text.as_bytes();
        let mut found = Vec::new();
        for dash in 0..bytes.len() {
            if bytes[dash] != b'-' || dash + 4 >= bytes.len() || bytes[dash + 4] != b'"' {
                continue;
            }
            if !bytes[dash + 1..dash + 4].iter().all(u8::is_ascii_digit) {
                continue;
            }
            let mut start = dash;
            while start > 0 && bytes[start - 1].is_ascii_uppercase() {
                start -= 1;
            }
            if dash - start < 2 || start == 0 || bytes[start - 1] != b'"' {
                continue;
            }
            found.push(&text[start..dash + 4]);
        }
        found
    }
}
