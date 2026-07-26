//! The package checker: lowers every declaration of a resolved package to IR
//! v2 and runs the typl composite and scalar checks plus the ridl interaction
//! checks (docs/ROADMAP.md epics E1.7a, E2.1b–c; typl language reference
//! §4–§12, §16; ridl language reference §3–§14, §16).
//!
//! Diagnostics accumulate; lowering continues past errors — the checker never
//! returns a hard error (ADR-0004 §5). Every check lowers as far as honesty
//! allows: an erroneous declaration still appears in the IR when its shape is
//! representable, with the diagnostic marking the failure. Duplicate
//! declarations lower once — exactly the resolver's first-wins winner
//! (ADR-0007 decision 6).
//!
//! Division of labour with the neighbouring passes:
//!
//! - the resolver ([`resolve_package`]) owns the module diagnostics
//!   (TYPL-00x); its diagnostics are **not** repeated here — a renderer merges
//!   [`Resolution::diagnostics`] and [`CheckedPackage::diagnostics`] itself
//!   (both use the same package-relative [`FileId`] scheme; see
//!   [`ridl_core::diag::remap_diagnostics`]);
//! - doc-comment semantics (`Decl::doc`, `labels`, `deprecated`) land with the
//!   task 14 doc scanner — this pass leaves them empty;
//! - every lowered `TypeDef` and `Field` carries a populated `InitValue`
//!   (typl §5.8, E1.9): the **declared** `= value` init (validated as TYPL-109)
//!   when present, otherwise the value derived from the §5.8 table by
//!   [`crate::init`]. A named type whose init is neither declared nor derivable
//!   is marked `{ derivable: false }` and reported as TYPL-115 (info).

use std::collections::{HashMap, HashSet};

use ridl_core::db::{InputFile, profile_of_path};
use ridl_core::diag::{DiagCode, Diagnostic, FileId, Label, Severity, SourceMap, Span};
use ridl_core::package::{Package, Workspace, package_of};
use ridl_ir::v2;
use ridl_syntax::ast::{self, AstNode, Definition, HasDocComments, HasModifiers, HasName};
use ridl_syntax::{Profile, SyntaxKind};
use rowan::{NodeOrToken, TextRange};

use crate::docs;
use crate::expr::{self, ContractScope, ExprType};
use crate::init;
use crate::lint;
use crate::resolve::{
    Resolution, Symbol, SymbolKind, declared_name, declared_symbols, name_range,
    qualified_segments, resolve_package, significant_text, source_file,
};
use crate::scalar::{
    DiagKind, ExactValue, FloatRange, IntRange, derive_float_width, derive_int_width,
    enumset_width, validate_range, validate_step,
};
use crate::timing;
use crate::ucum::parse_ucum;

/// A checked package: its lowered IR and the checker's own diagnostics.
///
/// The diagnostics' spans carry a [`FileId`] indexing `pkg.files(db)` in order
/// — the same package-relative scheme [`Resolution`] uses. The resolver's
/// diagnostics are not repeated here; a renderer reads both and remaps them
/// onto its own source map with [`ridl_core::diag::remap_diagnostics`].
#[derive(Debug, Clone, PartialEq)]
pub struct CheckedPackage {
    pub ir: v2::Package,
    pub diagnostics: Vec<Diagnostic>,
}

/// Checks `pkg` and lowers it to IR v2 (typl reference §4–§12, §16.2–§16.3).
///
/// `std` is the embedded `ridl.std` package, threaded in exactly as
/// [`resolve_package`] takes it (its constructor needs `&mut RidlDatabase`,
/// which a tracked query cannot hold).
#[salsa::tracked(returns(clone))]
pub fn check_package(
    db: &dyn salsa::Database,
    ws: Workspace,
    pkg: Package,
    std: Package,
) -> CheckedPackage {
    let resolution = resolve_package(db, ws, pkg, std);
    let package_name = pkg.name(db).clone();
    let files = pkg.files(db).clone();

    // Checker diagnostics point into the package's own files; their FileId
    // indexes pkg.files(db) in order (the Resolution scheme).
    let mut sources = SourceMap::new();
    let file_ids: Vec<FileId> = files
        .iter()
        .map(|file| sources.file_id(file.path(db), file.text(db)))
        .collect();

    // Resolve the package timing default once (ridl §9.1): the winning raw
    // `[defaults].timing` string (package `[defaults]` already shadows the
    // workspace default, merged at load) is parsed here — `ridl-core` cannot
    // depend on `ridl-sem`, so a malformed string is MANI-009, spanning the
    // package's first file, and the built-in `[100ms..1000ms]` is the fallback.
    let mut default_diagnostics = Vec::new();
    let default_timing = match pkg.default_timing(db).as_ref() {
        Some(raw) => match timing::parse_default_timing(raw) {
            Ok(spec) => spec,
            Err(reason) => {
                // The span is the package's first file. A package with no
                // interned files cannot carry one, so the diagnostic falls back
                // to the detached id (rendered as a bare coded message) rather
                // than vanishing — a dropped manifest error is worse than an
                // unanchored one.
                let file = file_ids.first().copied().unwrap_or(FileId::DETACHED);
                default_diagnostics.push(Diagnostic {
                    code: DiagCode::MANI_009,
                    severity: Severity::Error,
                    message: format!(
                        "invalid `[defaults].timing` in the package manifest: {reason}"
                    ),
                    primary: Span {
                        file,
                        range: TextRange::empty(0.into()),
                    },
                    labels: Vec::new(),
                    fixits: Vec::new(),
                });
                timing::builtin_default_timing()
            }
        },
        None => timing::builtin_default_timing(),
    };

    let mut checker = Checker {
        db,
        ws,
        std,
        pkg,
        package_name: package_name.clone(),
        resolution,
        file_ids,
        current_file: 0,
        diagnostics: default_diagnostics,
        default_timing,
        interface_signals: Vec::new(),
        interface_name: String::new(),
        interface_internal: false,
        contract_vocabulary: None,
    };

    let mut decls = Vec::new();
    let mut interfaces = Vec::new();
    let mut services = Vec::new();
    // Service dotted names already lowered in this package, so a same-package
    // duplicate lowers once (first-wins).
    let mut lowered_services: HashSet<String> = HashSet::new();
    let mut composite_starts = Vec::new();
    for (index, file) in files.iter().enumerate() {
        checker.current_file = index;
        let source = source_file(db, *file);
        for definition in source.definitions() {
            // First-wins: lower exactly the resolver's winner (ADR-0007
            // decision 6); a losing duplicate was already reported (TYPL-009).
            if !checker.is_winner(*file, &definition) {
                continue;
            }
            if matches!(&definition, Definition::Struct(_) | Definition::Union(_))
                && let Some(name) = declared_name(&definition)
            {
                composite_starts.push((name, index, name_range(&definition)));
            }
            if let Some(decl) = checker.lower_definition(&definition) {
                decls.push(decl);
            }
        }
        // The ridl interaction layer (E2.1b structural checks, E2.1c
        // lowering): interfaces land in `Package.interfaces` in source order.
        for interface in source.interfaces() {
            if !checker.is_winner(*file, &interface) {
                continue;
            }
            interfaces.push(checker.lower_interface(&interface));
        }
        // Services (E2.13): the global published declarations. Their dotted
        // names live in the workspace catalog namespace, not the type
        // namespace, so the resolver's `is_winner` does not apply — uniqueness
        // across the workspace is `service_catalog`'s job (RIDL-140). A
        // same-package duplicate still lowers only once, first-wins, so the IR
        // never carries a name twice while the catalog holds one entry.
        for service in source.services() {
            let name = service
                .name()
                .map(|dotted| significant_text(dotted.syntax()))
                .unwrap_or_default();
            // A service the parser recovered without a name does not lower,
            // for the reason a nameless interaction does not: a service is
            // published at its dotted global name, and the empty address is
            // not one. The parser has already reported FORM-101, so no second
            // diagnostic is raised here.
            if name.is_empty() || !lowered_services.insert(name) {
                continue;
            }
            services.push(checker.lower_service(&service));
        }
        // Visibility exposure (TYPL-005, RIDL-143) is a whole-file pass over
        // the top-level items rather than a step of each lowering, so no
        // declaration kind can escape it (issue #161).
        checker.check_exposure(&source);
        // Stream-position narrowing runs on ridl-profile files only: in a
        // `.typl` parse the parser itself reports every stream as TYPL-301.
        if profile_of_path(file.path(db)) == Profile::Ridl {
            checker.check_stream_positions(&source);
        }
    }

    checker.check_recursion(&composite_starts);

    // The ridl lint pass (E2.10a): four advisory codes over the interaction
    // declarations, emitted as ordinary diagnostics once lowering has settled.
    lint::lint_package(&mut checker, &files);

    CheckedPackage {
        ir: v2::Package {
            name: package_name,
            decls,
            interfaces,
            services,
        },
        diagnostics: checker.diagnostics,
    }
}

// ==========================================================================
// Constant evaluation (typl §6; Appendix E scalar rule — constants as bounds)
// ==========================================================================

/// A constant's value, resolved for use in range bounds and init values
/// (typl §6.1–§6.2): an exact numeric value, a boolean, a text value, or a
/// regex constant's source text (delimiters included).
///
/// Consumed by the init-derivation pass (task 15) through [`const_value`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConstValue {
    Number(ExactValue),
    Bool(bool),
    Text(String),
    Regex(String),
}

/// The value of the constant named `name`, looked up in `res` and evaluated by
/// following `const = const` chains to a literal (typl §6.1). Returns `None`
/// when the name is not a constant, its value is malformed, or the chain hits a
/// cycle.
///
/// Following a chain re-resolves each referenced constant in the package that
/// defines the constant holding the reference, so an imported constant is
/// evaluated in its own package's view. A `(package, name)` visited set breaks
/// cycles: a constant that references itself directly or transitively yields
/// `None` rather than looping forever.
///
/// Deviation from the task 14 interface, on record: the interface sketched
/// `const_value(res, name)`, but a constant's value lives in its source file,
/// reachable only through the salsa database — a bare `&Resolution` cannot read
/// it, and chain-following re-resolves packages. The database, workspace, and
/// embedded `ridl.std` are therefore threaded in. Task 15 runs inside
/// `check_package`, which already holds all three.
pub fn const_value(
    db: &dyn salsa::Database,
    ws: Workspace,
    std: Package,
    res: &Resolution,
    name: &str,
) -> Option<ConstValue> {
    let symbol = res.symbols.get(name)?.clone();
    let mut visited = HashSet::new();
    const_value_of_symbol(db, ws, std, symbol, &mut visited)
}

/// Evaluates the constant `symbol` names, following `const = const` chains.
/// `visited` carries the `(package, name)` pairs already seen, breaking cycles.
fn const_value_of_symbol(
    db: &dyn salsa::Database,
    ws: Workspace,
    std: Package,
    symbol: Symbol,
    visited: &mut HashSet<(String, String)>,
) -> Option<ConstValue> {
    if symbol.kind != SymbolKind::Const {
        return None;
    }
    if !visited.insert((symbol.package.clone(), symbol.name.clone())) {
        return None; // a const cycle — stop rather than loop
    }
    let source = source_file(db, symbol.file);
    let Definition::Const(decl) = source
        .definitions()
        .find(|definition| name_range(definition) == symbol.range)?
    else {
        return None;
    };
    match literal_kind(&decl.value()?) {
        LitKind::Number { value } => Some(ConstValue::Number(value)),
        LitKind::Bool(flag) => Some(ConstValue::Bool(flag)),
        LitKind::Str(text) => Some(ConstValue::Text(text)),
        LitKind::Regex(text) => Some(ConstValue::Regex(text)),
        LitKind::ConstRef(next) => {
            // The referenced constant resolves in the package that declares the
            // constant holding the reference (typl §3.2 name resolution).
            let package = const_package_handle(db, ws, std, &symbol.package)?;
            let resolution = resolve_package(db, ws, package, std);
            let next_symbol = resolution.symbols.get(&next)?.clone();
            const_value_of_symbol(db, ws, std, next_symbol, visited)
        }
        LitKind::Malformed => None,
    }
}

/// The package handle for a package name during constant evaluation: the
/// embedded `ridl.std` or a workspace member (which includes the package under
/// check — every member belongs to its own workspace).
fn const_package_handle(
    db: &dyn salsa::Database,
    ws: Workspace,
    std: Package,
    name: &str,
) -> Option<Package> {
    if name == std.name(db) {
        return Some(std);
    }
    package_of(db, ws, name.to_string())
}

// ==========================================================================
// The checker context
// ==========================================================================

pub(crate) struct Checker<'db> {
    pub(crate) db: &'db dyn salsa::Database,
    pub(crate) ws: Workspace,
    pub(crate) std: Package,
    pkg: Package,
    package_name: String,
    resolution: Resolution,
    file_ids: Vec<FileId>,
    pub(crate) current_file: usize,
    diagnostics: Vec<Diagnostic>,
    /// The resolved package timing default (ridl §9.1): the parsed
    /// `[defaults].timing` or the built-in `[100ms..1000ms]`, applied to every
    /// untimed signal and event (E2 task 9).
    default_timing: timing::TimingSpec,
    /// The signals of the interface being lowered, typed for the contract
    /// environment — the names a `require` may read (ridl §13, expr-core §6).
    /// Set by [`Checker::lower_interface`] before its members are lowered and
    /// cleared after; empty everywhere else.
    interface_signals: Vec<(String, ExprType)>,
    /// The name the observer stubs of the interface being lowered are scoped
    /// to (E2.5): the declared interface name, or a service's dotted global
    /// name for an inline shape, which has no name of its own. Set and cleared
    /// alongside [`Checker::interface_signals`].
    interface_name: String,
    /// Whether the interface being lowered is `internal`, read by
    /// [`Checker::check_clause_exposure`]. Set and cleared alongside
    /// [`Checker::interface_signals`]; `false` for a service's inline shape,
    /// which is always public (ridl §14.5).
    interface_internal: bool,
    /// The package's resolved constants and enums (expr-core §6), built on the
    /// first contract clause of the package and reused for the rest.
    contract_vocabulary: Option<expr::ContractVocabulary>,
}

/// The primitive class a scalar constraint is validated against.
#[derive(Clone, Copy, PartialEq, Eq)]
enum BackingClass {
    Boolean,
    Integer,
    /// `float` and every unit backing (§5.1: the underlying primitive of a
    /// unit type is `float`).
    Float,
    Str,
    Bytes,
    /// The backing failed to parse; no constraint checks apply.
    Unknown,
}

/// A lowered scalar constraint plus the exact bounds kept for init/const
/// validation.
struct ScalarParts {
    constraint: Option<v2::Constraint>,
    width: Option<v2::type_def::Width>,
    min: Option<ExactValue>,
    max: Option<ExactValue>,
}

impl ScalarParts {
    fn empty() -> Self {
        ScalarParts {
            constraint: None,
            width: None,
            min: None,
            max: None,
        }
    }
}

/// What a source literal holds.
enum LitKind {
    Number { value: ExactValue },
    ConstRef(String),
    Bool(bool),
    Str(String),
    Regex(String),
    Malformed,
}

/// Classifies a literal from its CST tokens. The raw token kind is kept —
/// `1` and `1.0` are one `ExactValue`, so form distinctions (the TYPL-105
/// integer-form-step check) must come from the token, never from the decimal
/// rendering.
fn literal_kind(literal: &ast::Literal) -> LitKind {
    if literal.true_token().is_some() {
        return LitKind::Bool(true);
    }
    if literal.false_token().is_some() {
        return LitKind::Bool(false);
    }
    if let Some(token) = literal.regex_token() {
        return LitKind::Regex(token.text().to_string());
    }
    if let Some(token) = literal.string_lit_token() {
        let text = token.text();
        let inner = text
            .strip_prefix('"')
            .and_then(|rest| rest.strip_suffix('"'))
            .unwrap_or(text);
        return LitKind::Str(inner.to_string());
    }
    if literal.int_number_token().is_some() || literal.float_number_token().is_some() {
        return match ExactValue::parse(&significant_text(literal.syntax())) {
            Some(value) => LitKind::Number { value },
            None => LitKind::Malformed,
        };
    }
    if let Some(token) = literal.ident_token() {
        return LitKind::ConstRef(token.text().to_string());
    }
    LitKind::Malformed
}

/// A resolved-or-not type reference.
enum PathTarget {
    Symbol(Symbol),
    /// Unresolved; carries the written text for honest lowering. The
    /// description-first diagnostic was already emitted.
    Unresolved(String),
}

/// The result of lowering one field type: the IR type plus the exact scalar
/// bounds when the type is a scalar (used to validate a declared init).
struct LoweredType {
    ty: v2::FieldType,
    /// Exact numeric bounds when the field type is a numeric scalar, used to
    /// validate a numeric declared init (TYPL-109).
    scalar_bounds: Option<(Option<ExactValue>, Option<ExactValue>)>,
    /// Length bounds and `match` pattern when the field type is a string/bytes
    /// scalar — inline (`name : string [0..8]`) or a named string/bytes `type`.
    /// Used to validate a declared string/bytes init (TYPL-109); the numeric
    /// path reads `scalar_bounds` instead.
    init_constraint: Option<v2::Constraint>,
}

impl LoweredType {
    fn plain(ty: v2::FieldType) -> Self {
        LoweredType {
            ty,
            scalar_bounds: None,
            init_constraint: None,
        }
    }
}

impl Checker<'_> {
    // --- plumbing ---------------------------------------------------------

    fn diag(&mut self, code: DiagCode, severity: Severity, range: TextRange, message: String) {
        self.diagnostics.push(Diagnostic {
            code,
            severity,
            message,
            primary: Span {
                file: self.file_ids[self.current_file],
                range,
            },
            labels: Vec::new(),
            fixits: Vec::new(),
        });
    }

    fn error(&mut self, code: DiagCode, range: TextRange, message: String) {
        self.diag(code, Severity::Error, range, message);
    }

    /// An error with one secondary label, pointing at the declaration the
    /// reader has to look at to understand the primary one — the shape
    /// RIDL-140 established for a name collision ("`x` is first declared
    /// here"). Both spans are in the file being checked.
    fn error_with_label(
        &mut self,
        code: DiagCode,
        range: TextRange,
        message: String,
        label_range: TextRange,
        label: String,
    ) {
        let file = self.file_ids[self.current_file];
        self.diagnostics.push(Diagnostic {
            code,
            severity: Severity::Error,
            message,
            primary: Span { file, range },
            labels: vec![Label {
                span: Span {
                    file,
                    range: label_range,
                },
                message: label,
            }],
            fixits: Vec::new(),
        });
    }

    pub(crate) fn warning(&mut self, code: DiagCode, range: TextRange, message: String) {
        self.diag(code, Severity::Warning, range, message);
    }

    pub(crate) fn info(&mut self, code: DiagCode, range: TextRange, message: String) {
        self.diag(code, Severity::Info, range, message);
    }

    /// Whether a declaration (typl definition or ridl interface) is the
    /// resolver's first-wins winner for its name — the one occurrence the
    /// checker processes (ADR-0007 decision 6).
    pub(crate) fn is_winner(&self, file: InputFile, declaration: &impl HasName) -> bool {
        let Some(name) = declared_name(declaration) else {
            return false;
        };
        match self.resolution.symbols.get(&name) {
            Some(symbol) => {
                symbol.package == self.package_name
                    && symbol.file == file
                    && symbol.range == name_range(declaration)
            }
            None => false,
        }
    }

    /// The package handle for a package name: the checked package itself, the
    /// embedded `ridl.std`, or a workspace member.
    pub(crate) fn package_handle(&self, name: &str) -> Option<Package> {
        if name == self.package_name {
            return Some(self.pkg);
        }
        if name == self.std.name(self.db) {
            return Some(self.std);
        }
        package_of(self.db, self.ws, name.to_string())
    }

    /// The IR canonical form of a resolved reference: bare for same-package,
    /// fully qualified for cross-package — never an import alias.
    pub(crate) fn canonical_ref(&self, symbol: &Symbol) -> String {
        if symbol.package == self.package_name {
            symbol.name.clone()
        } else {
            format!("{}.{}", symbol.package, symbol.name)
        }
    }

    /// Resolves a type path, emitting the T6 description-first messages
    /// (unknown name / non-type-where-type-expected) — no §16 code exists for
    /// either.
    fn resolve_type_path(&mut self, path: &ast::PathType) -> PathTarget {
        let written = significant_text(path.syntax());
        let range = path.syntax().text_range();
        match self.lookup_path(path) {
            Some(symbol) if symbol.kind == SymbolKind::Const => {
                self.error(
                    DiagCode::NONE,
                    range,
                    format!("expected a type, but `{written}` names a constant"),
                );
                PathTarget::Unresolved(written)
            }
            // An interface is not a type: it has no values, so it cannot sit
            // in payload, field, or parameter position (ridl §14.0).
            Some(symbol) if symbol.kind == SymbolKind::Interface => {
                self.error(
                    DiagCode::NONE,
                    range,
                    format!("expected a type, but `{written}` names an interface"),
                );
                PathTarget::Unresolved(written)
            }
            Some(symbol) => PathTarget::Symbol(symbol),
            None => {
                self.error(
                    DiagCode::NONE,
                    range,
                    format!("unknown type name `{written}`"),
                );
                PathTarget::Unresolved(written)
            }
        }
    }

    /// Resolves a path silently against the checked package's own view.
    pub(crate) fn lookup_path(&self, path: &ast::PathType) -> Option<Symbol> {
        self.lookup_path_in(&self.resolution, path)
    }

    /// Resolves a path silently in a given package view: a single segment is a
    /// bare name in that view; a longer path is a fully qualified
    /// `pkg.Name` reference (typl §3.2 — no import needed).
    pub(crate) fn lookup_path_in(
        &self,
        resolution: &Resolution,
        path: &ast::PathType,
    ) -> Option<Symbol> {
        let qualified = path.qualified_name()?;
        let mut segments = qualified_segments(&qualified);
        if segments.is_empty() {
            return None;
        }
        if segments.len() == 1 {
            return resolution.symbols.get(&segments[0]).cloned();
        }
        let name = segments.pop().expect("length checked above");
        let package_path = segments.join(".");
        let target = self.package_handle(&package_path)?;
        let symbol = declared_symbols(self.db, target).get(&name).cloned()?;
        // A foreign `internal` declaration is not visible (typl §3.3).
        if symbol.internal && symbol.package != self.package_name {
            return None;
        }
        Some(symbol)
    }

    /// Finds the AST definition a symbol points at (its file and name range).
    pub(crate) fn find_definition(&self, symbol: &Symbol) -> Option<Definition> {
        let source = source_file(self.db, symbol.file);
        source
            .definitions()
            .find(|definition| name_range(definition) == symbol.range)
    }

    /// The value of a constant, resolved in `package`'s context, following
    /// `const = const` chains through the free [`const_value`] (cycle-guarded).
    fn const_value_in(&self, package: Package, name: &str) -> Option<ConstValue> {
        let resolution = resolve_package(self.db, self.ws, package, self.std);
        const_value(self.db, self.ws, self.std, &resolution, name)
    }

    /// The exact numeric value of a constant in `package`'s context, or `None`
    /// when the name is missing, non-numeric, or cyclic. A chained constant
    /// (`const MAX = BASE`) resolves through [`Checker::const_value_in`].
    fn const_numeric_value_in(&self, package: Package, name: &str) -> Option<ExactValue> {
        match self.const_value_in(package, name)? {
            ConstValue::Number(value) => Some(value),
            _ => None,
        }
    }

    /// The regex source of a named regex constant, resolved in the checked
    /// package's view, plus its canonical reference.
    fn const_regex_value(&self, name: &str) -> Option<(String, String)> {
        let symbol = self.resolution.symbols.get(name)?.clone();
        if symbol.kind != SymbolKind::Const {
            return None;
        }
        let canonical = self.canonical_ref(&symbol);
        let Definition::Const(decl) = self.find_definition(&symbol)? else {
            return None;
        };
        match literal_kind(&decl.value()?) {
            LitKind::Regex(text) => Some((text, canonical)),
            _ => None,
        }
    }

    /// The numeric value of a scalar literal in the checked package's context:
    /// a number directly, or a named constant resolved through the const chain.
    /// Silent — a bound that must report an unresolved constant uses
    /// [`Checker::resolve_range_bound`] instead.
    fn numeric_literal(&self, literal: &ast::Literal) -> Option<ExactValue> {
        self.numeric_literal_in(self.pkg, literal)
    }

    fn numeric_literal_in(&self, package: Package, literal: &ast::Literal) -> Option<ExactValue> {
        match literal_kind(literal) {
            LitKind::Number { value } => Some(value),
            LitKind::ConstRef(name) => self.const_numeric_value_in(package, &name),
            _ => None,
        }
    }

    /// Resolves a range-bound literal (`min`, `max`, or `step`) to its exact
    /// value, emitting a diagnostic when a constant reference does not resolve
    /// to a number. §16.2 defines no code for a malformed bound constant: a
    /// referenced constant that exists but is non-numeric borrows TYPL-105 (the
    /// documented borrow — the type-mismatch arm), while an unknown or cyclic
    /// constant reference is the T6 description-first codeless message. Either
    /// way the width defers (the bound is written but unresolved) — the caller's
    /// `unresolved_bound` check still sees a `None`.
    fn resolve_range_bound(&mut self, literal: &ast::Literal) -> Option<ExactValue> {
        match literal_kind(literal) {
            LitKind::Number { value } => Some(value),
            LitKind::ConstRef(name) => match self.const_value_in(self.pkg, &name) {
                Some(ConstValue::Number(value)) => Some(value),
                Some(_) => {
                    self.error(
                        DiagCode::TYPL_105,
                        literal.syntax().text_range(),
                        format!("range bound constant `{name}` is not numeric"),
                    );
                    None
                }
                None => {
                    self.error(
                        DiagCode::NONE,
                        literal.syntax().text_range(),
                        format!("unknown constant `{name}` in range bound"),
                    );
                    None
                }
            },
            _ => None,
        }
    }

    // --- declarations -----------------------------------------------------

    fn lower_definition(&mut self, definition: &Definition) -> Option<v2::Decl> {
        let name = declared_name(definition)?;
        let kind = match definition {
            Definition::Type(decl) => v2::decl::Kind::TypeDef(self.lower_type(&name, decl)),
            Definition::Const(decl) => v2::decl::Kind::ConstDef(self.lower_const(&name, decl)),
            Definition::Struct(decl) => v2::decl::Kind::StructDef(self.lower_struct(decl)),
            Definition::Enum(decl) => v2::decl::Kind::EnumDef(self.lower_enum(decl)),
            Definition::EnumSet(decl) => v2::decl::Kind::EnumSetDef(self.lower_enum_set(decl)),
            Definition::Union(decl) => v2::decl::Kind::UnionDef(self.lower_union(decl)),
        };
        // TYPL-212: `error` is failure vocabulary for composites only
        // (typl §10.1) — struct, enum, union.
        if definition.is_error() && matches!(definition, Definition::Type(_) | Definition::Const(_))
        {
            let what = match definition {
                Definition::Type(_) => "type",
                _ => "const",
            };
            self.error(
                DiagCode::TYPL_212,
                name_range(definition),
                format!("`error` is not valid on a `{what}` declaration — only `struct`, `enum`, and `union`"),
            );
        }
        let visibility = if definition.is_internal() {
            v2::Visibility::Internal
        } else {
            v2::Visibility::Public
        };

        // Doc-comment body, @labels, and @deprecated (typl §14). TYPL-405 warns
        // when @deprecated carries no reason string; TYPL-404 warns on a blank
        // line between the doc comment and the definition (§16.5).
        let doc_info = docs::scan(&definition.doc_comments());
        if doc_info.deprecated_missing_reason() {
            self.warning(
                DiagCode::TYPL_405,
                name_range(definition),
                format!("`@deprecated` on `{name}` has no reason string"),
            );
        }
        if blank_line_before_definition(definition) {
            self.warning(
                DiagCode::TYPL_404,
                name_range(definition),
                format!("blank line between the doc comment and `{name}`"),
            );
        }

        Some(v2::Decl {
            name,
            visibility: visibility as i32,
            is_error: definition.is_error(),
            doc: doc_info.doc,
            labels: doc_info.labels,
            deprecated: doc_info.deprecated,
            // Package-level declarations carry no interaction ordinal
            // (ridl §11).
            ordinal: 0,
            kind: Some(kind),
        })
    }

    /// TYPL-005 and RIDL-143: a public declaration must not expose an
    /// `internal` one (typl §3.3, ridl §14.5). One pass over the file's
    /// top-level items, whatever their kind — which is the fix for issue #161.
    /// The rule used to be a step of the typl definition lowering, so the two
    /// declaration kinds E2 added, `interface` and `service`, were never
    /// checked at all: a public interface could carry an `internal` payload
    /// (the Rust backend then emits a `pub` trait over a `pub(crate)` type,
    /// which fails any consumer build under `-D warnings`) and a public service
    /// could publish an `internal` interface. Driving the pass off the syntax
    /// tree's own top-level children rather than off a list of lowering calls
    /// is what makes the omission unrepresentable: a declaration kind added
    /// later inherits the check without being wired to it.
    ///
    /// `package` and `import` headers name no type and carry no constraint, so
    /// they contribute no exposure position and need no exclusion of their own.
    ///
    /// Only same-package `internal` declarations are reachable — a foreign
    /// `internal` name never resolves (typl §3.3), so it cannot leak here. An
    /// `internal` item is exempt in full: package-private declarations may
    /// reference each other freely, and both sides then generate
    /// package-private code (ADR-0008 decision 7).
    ///
    /// Two known limits, both deliberate:
    ///
    /// - **Duplicate declarations report once each.** The lowering loop
    ///   consults `is_winner` and lowers only the resolver's winner (ADR-0007
    ///   decision 6); this pass has no typed node to ask, so a losing duplicate
    ///   that also crosses the boundary reports too. Every such diagnostic is a
    ///   true statement about the source, and the file already carries TYPL-009,
    ///   so the cost is one extra line on input that is rejected anyway —
    ///   cheaper than reintroducing a per-kind test to suppress it.
    /// - **An attribute value is not scanned.** `attr_value` parses a constant
    ///   reference, but every attribute key on an interaction currently draws
    ///   FORM-106/-107, so no such value survives to be published. ADR-0008
    ///   decision 13 anticipates a consumable key; the position becomes real
    ///   with the first one, and belongs in the [`ast::Literal`] arm below.
    fn check_exposure(&mut self, source: &ast::SourceFile) {
        for item in source.syntax().children() {
            if item
                .children_with_tokens()
                .any(|element| element.kind() == SyntaxKind::InternalKw)
            {
                continue;
            }
            // A declaration the parser recovered without a name does not lower
            // (FORM-101 is already reported), so it is not checked either.
            let Some(name) = item_declared_name(&item) else {
                continue;
            };
            self.report_exposures(&item, &name);
        }
    }

    /// Every `internal` name the public top-level item `item` reads, reported
    /// at the position that reads it. Three position families reach here:
    ///
    /// - a named-type reference ([`ast::PathType`]) — a typl field, arm, map
    ///   key or value, or enumset backing, and every ridl type position: a
    ///   signal, event or final payload, a command or query parameter, a query
    ///   return, a tuple-return field, an array element, a stream element, and
    ///   either arm of an inline `T | E`;
    /// - the same node in a service's shape position, where the resolved symbol
    ///   is an interface rather than a type — RIDL-143;
    /// - a bounds constant, an `Ident` inside a `Literal` under a scalar
    ///   `Constraint` or a collection length `Bound`.
    ///
    /// The fourth family — a constant or enum type named by a `require`/`ensure`
    /// clause — is **not** here. Deciding whether such a name resolves to the
    /// package vocabulary at all needs the clause's own scope, which binds
    /// parameters, `result` and (in a `require` only) the interface's signals
    /// ahead of it; that scope is built in [`Checker::lower_contracts`], so the
    /// check lives there and calls [`Checker::report_exposure`]. Re-deriving
    /// the binding order from the syntax here was tried and was wrong: it
    /// missed that an `ensure` scope carries no signals, so a signal named like
    /// an `internal` constant silently suppressed the diagnostic.
    fn report_exposures(&mut self, item: &ridl_syntax::SyntaxNode, decl_name: &str) {
        // Collect exposures first (immutable resolver reads), then report.
        let mut exposures: Vec<(DiagCode, TextRange, &'static str, String)> = Vec::new();
        for descendant in item.descendants() {
            if let Some(path) = ast::PathType::cast(descendant.clone()) {
                if let Some(symbol) = self.lookup_path(&path)
                    && symbol.internal
                    && symbol.package == self.package_name
                {
                    let range = path.syntax().text_range();
                    if symbol.kind == SymbolKind::Interface {
                        // A service's shape is the one position where an
                        // interface legally sits (ridl §14.5). Anywhere else
                        // `resolve_type_path` has already said that an
                        // interface is not a type, which is the defect to fix
                        // first, so exposure is not piled on top of it.
                        //
                        // This is the one kind-specific test in the pass, and
                        // it is a known limit: a later profile that introduces
                        // another interface-naming position — rsdl's
                        // `component … provides Iface` is the expected one —
                        // inherits the TYPL-005 arm of this walk but not
                        // RIDL-143, and will need its parent kind listed here.
                        // That is the forgotten-wiring shape this pass exists
                        // to remove, surviving in exactly one place because the
                        // grammar gives no way to ask "may an interface sit
                        // here?".
                        if path
                            .syntax()
                            .parent()
                            .is_some_and(|parent| parent.kind() == SyntaxKind::ServiceDef)
                        {
                            exposures.push((
                                DiagCode::RIDL_143,
                                range,
                                "interface",
                                symbol.name.clone(),
                            ));
                        }
                    } else {
                        exposures.push((DiagCode::TYPL_005, range, "type", symbol.name.clone()));
                    }
                }
                continue;
            }
            // A bounds constant (typl §3.3): an `Ident` inside a `Literal`
            // sitting in a scalar `Constraint` or a collection length `Bound`.
            // The two are structurally distinct nodes for one rule — a
            // constraint nests under the type, a length bound is a direct child
            // of the `ArrayType`/`MapType` — so both are named here. An init
            // value is deliberately neither: §3.3 lists bounds constants and
            // stops there.
            if let Some(literal) = ast::Literal::cast(descendant)
                && literal.syntax().ancestors().any(|ancestor| {
                    matches!(ancestor.kind(), SyntaxKind::Constraint | SyntaxKind::Bound)
                })
                && let LitKind::ConstRef(const_name) = literal_kind(&literal)
                && let Some(symbol) = self.resolution.symbols.get(&const_name)
                && symbol.internal
                && symbol.package == self.package_name
            {
                exposures.push((
                    DiagCode::TYPL_005,
                    literal.syntax().text_range(),
                    "constant",
                    symbol.name.clone(),
                ));
            }
        }
        for (code, range, noun, exposed) in exposures {
            self.report_exposure(code, range, noun, &exposed, decl_name);
        }
    }

    /// One exposure diagnostic. Shared by the syntax pass above and by
    /// [`Checker::lower_contracts`], so the two positions that can report an
    /// exposure cannot drift in wording or in code.
    fn report_exposure(
        &mut self,
        code: DiagCode,
        range: TextRange,
        noun: &str,
        exposed: &str,
        decl_name: &str,
    ) {
        let message = if code == DiagCode::RIDL_143 {
            format!(
                "service `{decl_name}` publishes internal {noun} `{exposed}` — a service is a global published contract and takes no `internal` modifier, so its shape must be public: drop `internal` from `{exposed}`, or give the service an inline shape (ridl §14.5)"
            )
        } else {
            format!(
                "public `{decl_name}` exposes internal {noun} `{exposed}` — a public declaration may name only public declarations, so that the contract surface is fully importable (typl §3.3)"
            )
        };
        self.error(code, range, message);
    }

    fn lower_type(&mut self, name: &str, decl: &ast::TypeDef) -> v2::TypeDef {
        let (backing, class) = self.lower_backing(decl.backing());
        let span = decl
            .backing()
            .map(|backing| backing.syntax().text_range())
            .unwrap_or_else(|| name_range(decl));
        let parts = self.lower_scalar(class, decl.constraint(), span);
        let (declared_init, declared) =
            self.lower_declared_init(decl.init_value(), &parts, DiagCode::TYPL_109);
        let mut type_def = v2::TypeDef {
            backing,
            constraint: parts.constraint,
            declared_init,
            init: declared,
            width: parts.width,
        };
        // E1.9: a type without a declared `= value` derives its init from the
        // §5.8 table. A named type whose init is not derivable (a string/bytes
        // type forbidding length 0, or a `match`-typed one) is reported as
        // TYPL-115 (info) — a consumer that requires an init escalates it.
        if type_def.init.is_none() {
            let derived = init::derive_type_init(&type_def);
            if !derived.derivable {
                self.info(
                    DiagCode::TYPL_115,
                    name_range(decl),
                    format!("type `{name}` has no derivable init value and no declared `= value`"),
                );
            }
            type_def.init = Some(derived);
        }
        type_def
    }

    fn lower_backing(
        &mut self,
        backing: Option<ast::Backing>,
    ) -> (Option<v2::Backing>, BackingClass) {
        match backing {
            None => (None, BackingClass::Unknown),
            Some(ast::Backing::Primitive(node)) => {
                let (primitive, class) = primitive_of(&node);
                (
                    Some(v2::Backing {
                        kind: Some(v2::backing::Kind::Primitive(primitive as i32)),
                    }),
                    class,
                )
            }
            Some(ast::Backing::Unit(node)) => {
                let written = significant_text(node.syntax());
                let unit = match parse_ucum(&written) {
                    Ok(expr) => expr.canonical,
                    Err(_) => {
                        // TYPL-110; the raw source text is carried so tools can
                        // still show what was written.
                        self.error(
                            DiagCode::TYPL_110,
                            node.syntax().text_range(),
                            format!("unknown or malformed UCUM unit expression `{written}`"),
                        );
                        written
                    }
                };
                (
                    Some(v2::Backing {
                        kind: Some(v2::backing::Kind::Unit(unit)),
                    }),
                    BackingClass::Float,
                )
            }
        }
    }

    fn lower_const(&mut self, name: &str, decl: &ast::ConstDef) -> v2::ConstDef {
        let value_literal = decl.value();
        let value_kind = value_literal.as_ref().map(literal_kind);
        let value_range = value_literal
            .as_ref()
            .map(|literal| literal.syntax().text_range())
            .unwrap_or_default();

        // The declared type: a primitive keyword as written, or a named-type
        // reference in canonical form. Absent for regex constants. The resolved
        // named-type symbol is kept for the §5.7 nominal identity check.
        let mut bounds: Option<(Option<ExactValue>, Option<ExactValue>, String)> = None;
        let mut target_type: Option<Symbol> = None;
        let type_ref = decl.type_ref().map(|path| {
            if let Some(keyword) = primitive_path_keyword(&path) {
                return keyword;
            }
            match self.resolve_type_path(&path) {
                PathTarget::Symbol(symbol) => {
                    let canonical = self.canonical_ref(&symbol);
                    if symbol.kind == SymbolKind::Type {
                        if let Some((min, max)) = self.named_scalar_bounds(&symbol) {
                            bounds = Some((min, max, canonical.clone()));
                        }
                        target_type = Some(symbol.clone());
                    }
                    canonical
                }
                PathTarget::Unresolved(written) => written,
            }
        });

        // TYPL-108: a numeric const value must satisfy its type's range.
        if let (Some(LitKind::Number { value }), Some((min, max, type_name))) =
            (&value_kind, &bounds)
            && out_of_bounds(value, min.as_ref(), max.as_ref())
        {
            self.error(
                DiagCode::TYPL_108,
                value_range,
                format!(
                    "const `{name}` value {} outside `{type_name}` range [{}, {}]",
                    value.to_decimal_string(),
                    render_bound(min.as_ref()),
                    render_bound(max.as_ref()),
                ),
            );
        }

        // TYPL-108, §5.7 nominal identity: a const of a named type is never
        // initialized from a value of another named type. A value that
        // references a const of a different named type is the reachable case.
        let nominal_reported = if let (Some(LitKind::ConstRef(source_name)), Some(target)) =
            (&value_kind, &target_type)
            && let Some((target_display, source_display)) =
                self.nominal_init_violation(target, source_name)
        {
            self.error(
                DiagCode::TYPL_108,
                value_range,
                format!(
                    "const `{name}` of type `{target_display}` cannot be initialized from `{source_name}` of type `{source_display}` — nominal types are not interchangeable (§5.7)",
                ),
            );
            true
        } else {
            false
        };

        // TYPL-108: a value that references another constant must still satisfy
        // this constant's declared range. Only direct numeric literals reach
        // the range check above; a `const = const` init resolves through the
        // const chain here. Skipped when a nominal-identity violation already
        // fired, so one init reports one TYPL-108.
        if !nominal_reported
            && let (Some(LitKind::ConstRef(source_name)), Some((min, max, type_name))) =
                (&value_kind, &bounds)
            && let Some(value) = self.const_numeric_value_in(self.pkg, source_name)
            && out_of_bounds(&value, min.as_ref(), max.as_ref())
        {
            self.error(
                DiagCode::TYPL_108,
                value_range,
                format!(
                    "const `{name}` value {} outside `{type_name}` range [{}, {}]",
                    value.to_decimal_string(),
                    render_bound(min.as_ref()),
                    render_bound(max.as_ref()),
                ),
            );
        }

        // TYPL-106: a regex constant's pattern must be a valid ECMA-262 regex.
        if let Some(LitKind::Regex(text)) = &value_kind {
            self.validate_regex(text, value_range);
        }

        // `ConstDef.value` is a VALUE, never a reference: the IR carries no
        // discriminator between the two, so a reference left in it is
        // indistinguishable from a string constant that happens to spell the
        // same name (`const A : string = "B"` and `const A : string = B` lower
        // identically). A consumer therefore cannot resolve one, and every one
        // of them got it wrong differently — the Rust backend emitted
        // `Tick(SECRET)`, which does not compile, and `"SECRET"`, which
        // compiles and is the wrong value; the TypeScript backend refused the
        // package outright with a message about `Number.MAX_SAFE_INTEGER`
        // (issue #170).
        //
        // So the chain is followed here, through the same [`const_value`] the
        // range bounds, the `match` patterns and [`Self::declared_type_init`]
        // already use — one resolution, cycle-guarded, and correct across
        // packages, which a backend holding one package's IR could not be.
        // A reference that does not resolve keeps the written text, exactly as
        // before: the diagnostic for it belongs to the resolver.
        let value_kind = match value_kind {
            Some(LitKind::ConstRef(name)) => match self.const_value_in(self.pkg, &name) {
                Some(ConstValue::Number(value)) => Some(LitKind::Number { value }),
                Some(ConstValue::Bool(flag)) => Some(LitKind::Bool(flag)),
                Some(ConstValue::Text(text)) => Some(LitKind::Str(text)),
                Some(ConstValue::Regex(text)) => Some(LitKind::Regex(text)),
                None => Some(LitKind::ConstRef(name)),
            },
            other => other,
        };

        let (value, regex) = match value_kind {
            Some(LitKind::Number { value }) => (value.to_decimal_string(), None),
            Some(LitKind::Bool(flag)) => (flag.to_string(), None),
            Some(LitKind::Str(text)) => (text, None),
            Some(LitKind::Regex(text)) => (String::new(), Some(text)),
            Some(LitKind::ConstRef(_)) | Some(LitKind::Malformed) | None => (
                value_literal
                    .as_ref()
                    .map(|literal| significant_text(literal.syntax()))
                    .unwrap_or_default(),
                None,
            ),
        };
        v2::ConstDef {
            type_ref,
            value,
            regex,
        }
    }

    /// The exact numeric bounds of a named scalar type, read at its
    /// definition site; constant bounds resolve in the defining package's
    /// context.
    fn named_scalar_bounds(
        &self,
        symbol: &Symbol,
    ) -> Option<(Option<ExactValue>, Option<ExactValue>)> {
        let Definition::Type(decl) = self.find_definition(symbol)? else {
            return None;
        };
        let constraint = decl.constraint()?;
        let package = self.package_handle(&symbol.package)?;
        let min = constraint
            .min()
            .and_then(|literal| self.numeric_literal_in(package, &literal));
        let max = constraint
            .max()
            .and_then(|literal| self.numeric_literal_in(package, &literal));
        Some((min, max))
    }

    /// The nominal violation for `const target = source_name`, if any: the
    /// display names of the two named types when `source_name` is a constant of
    /// a named type distinct from `target`'s (§5.7, TYPL-108). `None` when the
    /// source is not a constant, is not named-typed, or shares `target`'s type.
    fn nominal_init_violation(
        &self,
        target: &Symbol,
        source_name: &str,
    ) -> Option<(String, String)> {
        let source = self.resolution.symbols.get(source_name)?;
        if source.kind != SymbolKind::Const {
            return None;
        }
        let source_type = self.const_named_type(source)?;
        if (source_type.package.as_str(), source_type.name.as_str())
            == (target.package.as_str(), target.name.as_str())
        {
            return None;
        }
        Some((self.canonical_ref(target), self.canonical_ref(&source_type)))
    }

    /// The named type a constant declares (`const X : T`, T a named type), as
    /// the resolved type [`Symbol`], resolved in the constant's own defining
    /// package. `None` when the constant is primitive-typed, untyped, or its
    /// type does not resolve to a named type.
    fn const_named_type(&self, symbol: &Symbol) -> Option<Symbol> {
        let Definition::Const(decl) = self.find_definition(symbol)? else {
            return None;
        };
        let path = decl.type_ref()?;
        if primitive_path_keyword(&path).is_some() {
            return None;
        }
        let package = self.package_handle(&symbol.package)?;
        let resolution = resolve_package(self.db, self.ws, package, self.std);
        match self.lookup_path_in(&resolution, &path) {
            Some(type_symbol) if type_symbol.kind == SymbolKind::Type => Some(type_symbol),
            _ => None,
        }
    }

    /// Validates a regex literal's pattern with the `regress` ECMA-262 engine
    /// (typl §2.7; ADR-0007 decision 10), emitting TYPL-106 on invalid syntax.
    /// A typl regex literal carries its `/…/` delimiters; the engine parses the
    /// body between them.
    fn validate_regex(&mut self, raw: &str, range: TextRange) {
        if regress::Regex::new(regex_body(raw)).is_err() {
            self.error(
                DiagCode::TYPL_106,
                range,
                "invalid regular expression syntax".to_string(),
            );
        }
    }

    // --- scalar constraints -----------------------------------------------

    fn lower_scalar(
        &mut self,
        class: BackingClass,
        constraint: Option<ast::Constraint>,
        decl_span: TextRange,
    ) -> ScalarParts {
        match class {
            // §4.1: boolean has no constraint syntax; §16 defines no code for
            // a stray one, so it is ignored here.
            BackingClass::Boolean | BackingClass::Unknown => ScalarParts::empty(),
            BackingClass::Integer => self.lower_integer_scalar(constraint, decl_span),
            BackingClass::Float => self.lower_float_scalar(constraint, decl_span),
            BackingClass::Str => self.lower_len_scalar("string", true, constraint, decl_span),
            BackingClass::Bytes => self.lower_len_scalar("bytes", false, constraint, decl_span),
        }
    }

    fn lower_integer_scalar(
        &mut self,
        constraint: Option<ast::Constraint>,
        decl_span: TextRange,
    ) -> ScalarParts {
        let Some(constraint) = constraint else {
            self.warning(
                DiagCode::TYPL_101,
                decl_span,
                "`integer` without a range constraint".to_string(),
            );
            return ScalarParts {
                constraint: None,
                // §4.2 last row: no range derives int64.
                width: Some(v2::type_def::Width::IntWidth(v2::IntWidth::I64 as i32)),
                min: None,
                max: None,
            };
        };
        let min_literal = constraint.min();
        let max_literal = constraint.max();
        let min = min_literal
            .as_ref()
            .and_then(|literal| self.resolve_range_bound(literal));
        let max = max_literal
            .as_ref()
            .and_then(|literal| self.resolve_range_bound(literal));
        // A bound that is written but does not resolve numerically (an unknown,
        // cyclic, or non-numeric constant reference) is not an omitted bound:
        // taking the §5.5 default would derive a definite width that flips when
        // the constant later resolves — a silent wire break. The width defers
        // instead (the TYPL-111 shape); `resolve_range_bound` has already
        // reported the unresolved constant.
        let unresolved_bound =
            (min_literal.is_some() && min.is_none()) || (max_literal.is_some() && max.is_none());

        // TYPL-105: `step` quantizes floats only (§4.3).
        if let Some(step) = constraint.step() {
            self.error(
                DiagCode::TYPL_105,
                step.syntax().text_range(),
                "`step` is not valid on an `integer` type".to_string(),
            );
        }
        if let (Some(min), Some(max)) = (&min, &max)
            && validate_range(min, max).is_some()
        {
            self.error(
                DiagCode::TYPL_104,
                constraint.syntax().text_range(),
                format!(
                    "range minimum {} is greater than maximum {}",
                    min.to_decimal_string(),
                    max.to_decimal_string()
                ),
            );
        }

        // §5.5: a bound genuinely omitted from source defaults to the widest
        // value the inferred width allows — the int64 domain edge on the
        // open side.
        let width = if unresolved_bound {
            None
        } else {
            let effective_min = min.clone().unwrap_or_else(|| int64_edge(false));
            let effective_max = max.clone().unwrap_or_else(|| int64_edge(true));
            match derive_int_width(&IntRange {
                min: effective_min,
                max: effective_max,
            }) {
                Ok(width) => Some(v2::type_def::Width::IntWidth(
                    v2::IntWidth::from(width) as i32
                )),
                Err(error) => {
                    debug_assert_eq!(error.code(), "TYPL-111");
                    self.error(
                        DiagCode::TYPL_111,
                        constraint.syntax().text_range(),
                        "integer range bound outside the `int64` domain `[-2^63..2^63-1]`"
                            .to_string(),
                    );
                    None
                }
            }
        };
        ScalarParts {
            constraint: Some(v2::Constraint {
                min: min.as_ref().map(ExactValue::to_decimal_string),
                max: max.as_ref().map(ExactValue::to_decimal_string),
                step: None,
                len_min: None,
                len_max: None,
                pattern: None,
                pattern_const: None,
            }),
            width,
            min,
            max,
        }
    }

    fn lower_float_scalar(
        &mut self,
        constraint: Option<ast::Constraint>,
        decl_span: TextRange,
    ) -> ScalarParts {
        let missing_constraint_warning = |checker: &mut Self, span| {
            checker.warning(
                DiagCode::TYPL_102,
                span,
                "`float` without both a range and a `step`".to_string(),
            );
        };
        let Some(constraint) = constraint else {
            missing_constraint_warning(self, decl_span);
            return ScalarParts {
                constraint: None,
                width: Some(v2::type_def::Width::FloatWidth(v2::FloatWidth::F64 as i32)),
                min: None,
                max: None,
            };
        };
        let min_literal = constraint.min();
        let max_literal = constraint.max();
        let min = min_literal
            .as_ref()
            .and_then(|literal| self.resolve_range_bound(literal));
        let max = max_literal
            .as_ref()
            .and_then(|literal| self.resolve_range_bound(literal));

        let step_literal = constraint.step();
        let step = step_literal
            .as_ref()
            .and_then(|literal| self.resolve_range_bound(literal));
        // Same rule as the integer path: a written-but-unresolved bound or
        // step must not silently derive a definite width — defer it. The
        // unresolved constant has already been reported by
        // `resolve_range_bound`.
        let unresolved_bound = (min_literal.is_some() && min.is_none())
            || (max_literal.is_some() && max.is_none())
            || (step_literal.is_some() && step.is_none());
        // The T10 review fact: `1` and `1.0` erase to one ExactValue, so the
        // integer-form check reads the raw CST token kind.
        if let Some(literal) = &step_literal
            && literal.int_number_token().is_some()
        {
            self.error(
                DiagCode::TYPL_105,
                literal.syntax().text_range(),
                "integer-form `step` on a `float` type (write `1.0`, not `1`)".to_string(),
            );
        }

        if let (Some(min), Some(max)) = (&min, &max)
            && validate_range(min, max).is_some()
        {
            self.error(
                DiagCode::TYPL_104,
                constraint.syntax().text_range(),
                format!(
                    "range minimum {} is greater than maximum {}",
                    min.to_decimal_string(),
                    max.to_decimal_string()
                ),
            );
        }

        let float_range = |step: Option<ExactValue>| FloatRange {
            min: min
                .clone()
                .unwrap_or_else(|| ExactValue::parse("0").expect("0 parses")),
            max: max
                .clone()
                .unwrap_or_else(|| ExactValue::parse("0").expect("0 parses")),
            step,
        };
        if let (Some(_), Some(_), Some(step_value)) = (&min, &max, &step) {
            if let Some(kind) = validate_step(&float_range(Some(step_value.clone()))) {
                debug_assert_eq!(kind.code(), "TYPL-105");
                let message = match kind {
                    DiagKind::StepNonPositive => "`step` must be positive",
                    DiagKind::StepLargerThanRange => "`step` is larger than the range",
                    _ => "`step` does not fit the range",
                };
                self.error(
                    DiagCode::TYPL_105,
                    step_literal
                        .as_ref()
                        .map(|literal| literal.syntax().text_range())
                        .unwrap_or_else(|| constraint.syntax().text_range()),
                    message.to_string(),
                );
            }
        } else if let Some(step_value) = &step
            && !exact_is_positive(step_value)
        {
            self.error(
                DiagCode::TYPL_105,
                step_literal
                    .as_ref()
                    .map(|literal| literal.syntax().text_range())
                    .unwrap_or_else(|| constraint.syntax().text_range()),
                "`step` must be positive".to_string(),
            );
        }

        // §4 recommendation: a float wants both a range and a step. Judged on
        // the written literals — a bound that is written but unresolved is
        // declared, not missing.
        if min_literal.is_none() || max_literal.is_none() || step_literal.is_none() {
            missing_constraint_warning(self, decl_span);
        }

        let width = if unresolved_bound {
            None
        } else {
            let width = if min.is_some() && max.is_some() {
                derive_float_width(&float_range(step.clone()))
            } else {
                crate::scalar::FloatWidth::F64
            };
            Some(v2::type_def::Width::FloatWidth(
                v2::FloatWidth::from(width) as i32
            ))
        };
        ScalarParts {
            constraint: Some(v2::Constraint {
                min: min.as_ref().map(ExactValue::to_decimal_string),
                max: max.as_ref().map(ExactValue::to_decimal_string),
                step: step.as_ref().map(ExactValue::to_decimal_string),
                len_min: None,
                len_max: None,
                pattern: None,
                pattern_const: None,
            }),
            width,
            min,
            max,
        }
    }

    fn lower_len_scalar(
        &mut self,
        noun: &str,
        allow_pattern: bool,
        constraint: Option<ast::Constraint>,
        decl_span: TextRange,
    ) -> ScalarParts {
        let len_bound = constraint.as_ref().and_then(ast::Constraint::len);
        let (len_min, len_max) = match &len_bound {
            Some(bound) => {
                let min = bound
                    .min()
                    .and_then(|literal| self.numeric_literal(&literal))
                    .and_then(|value| exact_to_u64(&value));
                let max = bound
                    .max()
                    .and_then(|literal| self.numeric_literal(&literal))
                    .and_then(|value| exact_to_u64(&value));
                if bound.dotdot_token().is_some() {
                    (min.unwrap_or(0), max.unwrap_or(256))
                } else {
                    // A single length is a fixed bound: `[N]` (§5.3–§5.4).
                    let n = min.unwrap_or(0);
                    (n, n)
                }
            }
            None => {
                // §4.4–§4.5: the `[0..256]` default applies, with a warning.
                self.warning(
                    DiagCode::TYPL_103,
                    decl_span,
                    format!("`{noun}` without explicit bounds; the default `[0..256]` applies"),
                );
                (0, 256)
            }
        };

        let mut pattern = None;
        let mut pattern_const = None;
        if allow_pattern
            && let Some(match_literal) =
                constraint.as_ref().and_then(ast::Constraint::match_pattern)
        {
            match literal_kind(&match_literal) {
                LitKind::Regex(text) => {
                    // TYPL-106: an inline `match` regex is validated here; a
                    // named regex constant is validated at its own declaration.
                    self.validate_regex(&text, match_literal.syntax().text_range());
                    pattern = Some(text);
                }
                LitKind::ConstRef(name) => match self.const_regex_value(&name) {
                    Some((text, canonical)) => {
                        pattern = Some(text);
                        pattern_const = Some(canonical);
                    }
                    // An unresolved or non-regex pattern constant: carry the
                    // name. Its regex validity (TYPL-106) is checked where the
                    // constant is declared.
                    None => pattern_const = Some(name),
                },
                _ => {}
            }
        }

        ScalarParts {
            constraint: Some(v2::Constraint {
                min: None,
                max: None,
                step: None,
                len_min: Some(len_min),
                len_max: Some(len_max),
                pattern,
                pattern_const,
            }),
            width: None,
            min: None,
            max: None,
        }
    }

    /// Lowers a declared `= value` init, validating it against the scalar
    /// bounds (TYPL-109): a numeric init against the numeric range, a
    /// string/bytes init against the length bound and, where the type carries a
    /// `match` pattern, against that pattern (see [`Checker::check_string_init`]).
    /// Returns `(declared_init, init)`; both stay absent when no init is
    /// declared — derivation is the task 15 pass.
    /// `violation` is the code an out-of-constraint init draws: TYPL-109 at
    /// the vocabulary layer (types and fields), RIDL-110 for a signal's
    /// `= value` override (ridl §4.4, E2 task 5) — one validation, two codes.
    fn lower_declared_init(
        &mut self,
        init: Option<ast::InitValue>,
        parts: &ScalarParts,
        violation: DiagCode,
    ) -> (Option<String>, Option<v2::InitValue>) {
        let Some(literal) = init.as_ref().and_then(ast::InitValue::literal) else {
            return (None, None);
        };
        let text = match literal_kind(&literal) {
            LitKind::Number { value } => {
                if out_of_bounds(&value, parts.min.as_ref(), parts.max.as_ref()) {
                    self.error(
                        violation,
                        literal.syntax().text_range(),
                        format!(
                            "init value {} is outside the declared range [{}..{}]",
                            value.to_decimal_string(),
                            render_bound(parts.min.as_ref()),
                            render_bound(parts.max.as_ref()),
                        ),
                    );
                }
                value.to_decimal_string()
            }
            LitKind::Bool(flag) => flag.to_string(),
            LitKind::Str(text) => {
                self.check_string_init(&text, parts, literal.syntax().text_range(), violation);
                text
            }
            // A constant is reusable in an init (§6), so the value that lowers
            // is the constant's, not its name. Every kind resolves, not only
            // the numeric one: `= SOME_TEXT` used to lower as the literal text
            // `"SOME_TEXT"` and `= SOME_FLAG` as the unparseable `"YES"`, which
            // both backends then emitted as a wrong value with no diagnostic
            // anywhere (issue #170). The resolved value is checked against the
            // declared bounds exactly as a direct literal of the same kind is.
            LitKind::ConstRef(name) => match self.const_value_in(self.pkg, &name) {
                Some(ConstValue::Number(value)) => {
                    if out_of_bounds(&value, parts.min.as_ref(), parts.max.as_ref()) {
                        self.error(
                            violation,
                            literal.syntax().text_range(),
                            format!(
                                "init value {} is outside the declared range [{}..{}]",
                                value.to_decimal_string(),
                                render_bound(parts.min.as_ref()),
                                render_bound(parts.max.as_ref()),
                            ),
                        );
                    }
                    value.to_decimal_string()
                }
                Some(ConstValue::Bool(flag)) => flag.to_string(),
                Some(ConstValue::Text(text)) => {
                    self.check_string_init(&text, parts, literal.syntax().text_range(), violation);
                    text
                }
                Some(ConstValue::Regex(text)) => text,
                None => significant_text(literal.syntax()),
            },
            LitKind::Regex(text) => text,
            LitKind::Malformed => significant_text(literal.syntax()),
        };
        (
            Some(text.clone()),
            Some(v2::InitValue {
                derivable: true,
                value: Some(text),
            }),
        )
    }

    /// Checks a declared string/bytes init against the type's length bound and,
    /// where a `match` pattern is present, against that pattern (TYPL-109,
    /// §5.8). Length is measured in Unicode scalar values; the string's own
    /// escape processing is not applied (the raw inner text is measured).
    /// Pattern conformance uses ECMA-262 `test` semantics — a match anywhere in
    /// the string; typl patterns are anchored with `^`…`$`. An invalid pattern
    /// is skipped here (TYPL-106 reports it at the pattern's own site). Reached
    /// with a populated `constraint` for a named `type` and, since E1.9, for a
    /// struct field too: the length bound and `match` pattern flow into the
    /// field's `ScalarParts` whether the field is an inline string/bytes scalar
    /// or is typed by a named string/bytes `type` (see [`Checker::lower_field`]).
    /// `violation` is the emitted code — see [`Checker::lower_declared_init`].
    fn check_string_init(
        &mut self,
        text: &str,
        parts: &ScalarParts,
        range: TextRange,
        violation: DiagCode,
    ) {
        let Some(constraint) = &parts.constraint else {
            return;
        };
        let length = text.chars().count() as u64;
        if let Some(min) = constraint.len_min
            && length < min
        {
            self.error(
                violation,
                range,
                format!("init string length {length} is below the declared minimum {min}"),
            );
        } else if let Some(max) = constraint.len_max
            && length > max
        {
            self.error(
                violation,
                range,
                format!("init string length {length} exceeds the declared maximum {max}"),
            );
        }
        if let Some(pattern) = &constraint.pattern
            && let Ok(regex) = regress::Regex::new(regex_body(pattern))
            && regex.find(text).is_none()
        {
            self.error(
                violation,
                range,
                format!("init string `{text}` does not match the type's `match` pattern"),
            );
        }
    }

    // --- structs ----------------------------------------------------------

    fn lower_struct(&mut self, decl: &ast::StructDef) -> v2::StructDef {
        // Pre-pass: the reserved tombstones, for TYPL-210/211.
        let mut reserved_names: HashSet<String> = HashSet::new();
        let mut reserved_values: HashSet<i64> = HashSet::new();
        for member in decl.members() {
            if let ast::StructMember::Reserved(entry) = member {
                self.record_reserved(&entry, &mut reserved_names, &mut reserved_values);
            }
        }

        let mut members = Vec::new();
        let mut ordinal = 0u32;
        let mut fixed = true;
        for member in decl.members() {
            ordinal += 1;
            match member {
                ast::StructMember::Reserved(entry) => {
                    // A tombstone implies evolution; an evolved struct is
                    // never emitted as a fixed inline layout.
                    fixed = false;
                    members.push(v2::StructMember {
                        member: Some(v2::struct_member::Member::Reserved(lower_reserved(
                            &entry, ordinal,
                        ))),
                    });
                }
                ast::StructMember::Field(field) => {
                    if let Some(name) = member_name(field.name())
                        && reserved_names.contains(&name)
                    {
                        self.error(
                            DiagCode::TYPL_210,
                            member_name_range(field.name(), field.syntax()),
                            format!("field `{name}` re-declares a `reserved` name"),
                        );
                    }
                    let lowered = self.lower_field(&field, ordinal);
                    if lowered
                        .r#type
                        .as_ref()
                        .is_none_or(|ty| ty.optional || !self.lowered_source_is_fixed(&field))
                    {
                        fixed = false;
                    }
                    members.push(v2::StructMember {
                        member: Some(v2::struct_member::Member::Field(lowered)),
                    });
                }
            }
        }
        v2::StructDef {
            members,
            fixed_layout: fixed && ordinal > 0,
        }
    }

    /// Whether a field's declared type is fixed-width, judged from the source
    /// in the checked package's context.
    fn lowered_source_is_fixed(&self, field: &ast::FieldDef) -> bool {
        let Some(field_type) = field.field_type() else {
            return false;
        };
        let mut visiting = HashSet::new();
        self.field_type_is_fixed(&self.resolution, &field_type, &mut visiting)
    }

    fn lower_field(&mut self, field: &ast::FieldDef, ordinal: u32) -> v2::Field {
        let name = member_name(field.name()).unwrap_or_default();
        let lowered = field
            .field_type()
            .map(|field_type| self.lower_field_type(&field_type, false));
        // The field's own bounds validate a declared init (TYPL-109): the exact
        // numeric bounds for a numeric field, and the length bounds plus `match`
        // pattern for a string/bytes field — inline or through a named string
        // `type`. Both come from the lowered field type.
        let bounds_parts = ScalarParts {
            constraint: lowered.as_ref().and_then(|l| l.init_constraint.clone()),
            width: None,
            min: lowered
                .as_ref()
                .and_then(|l| l.scalar_bounds.as_ref())
                .and_then(|(min, _)| min.clone()),
            max: lowered
                .as_ref()
                .and_then(|l| l.scalar_bounds.as_ref())
                .and_then(|(_, max)| max.clone()),
        };
        let (declared_init, declared) =
            self.lower_declared_init(field.init_value(), &bounds_parts, DiagCode::TYPL_109);
        // E1.9: a field without a declared init derives one from the §5.8 table
        // (a named reference resolves to the referenced type's own init).
        let init = match declared {
            Some(init) => Some(init),
            None => lowered.as_ref().map(|l| self.derive_field_init(&l.ty)),
        };
        v2::Field {
            name,
            ordinal,
            r#type: lowered.map(|l| l.ty),
            declared_init,
            init,
            doc: String::new(),
            labels: Vec::new(),
            deprecated: None,
        }
    }

    /// Derives a field's init from its lowered field type (typl §5.8), resolving
    /// a named reference to the referenced type's own init. See [`crate::init`].
    fn derive_field_init(&self, field_type: &v2::FieldType) -> v2::InitValue {
        init::derive_field_init(field_type, &|name| self.named_ref_init(name))
    }

    /// The derived init of a named reference (`Speed`, `ridl.std.Name`), by kind
    /// (typl §5.8): a scalar `type` materializes its value; an `enum` its `0`
    /// or lowest value; an `enumset` the empty set; a `struct` or `union` is a
    /// derivable composite the consumer reconstructs (a `union` inherits its
    /// first arm's derivability). An unresolved reference — already reported by
    /// the type-resolution pass — is treated as a derivable composite.
    fn named_ref_init(&self, canonical: &str) -> v2::InitValue {
        match self.resolve_canonical(canonical) {
            Some(symbol) => self.named_type_init(&symbol),
            None => v2::InitValue {
                derivable: true,
                value: None,
            },
        }
    }

    /// The derived init of a resolved named type (typl §5.8).
    fn named_type_init(&self, symbol: &Symbol) -> v2::InitValue {
        match symbol.kind {
            SymbolKind::Type => {
                let Some(Definition::Type(decl)) = self.find_definition(symbol) else {
                    return v2::InitValue {
                        derivable: true,
                        value: None,
                    };
                };
                // A named type with its own declared init carries that value.
                if let Some(init) = self.declared_type_init(&decl, symbol) {
                    return init;
                }
                match backing_class(decl.backing()) {
                    BackingClass::Boolean => v2::InitValue {
                        derivable: true,
                        value: Some("false".to_string()),
                    },
                    BackingClass::Integer | BackingClass::Float => {
                        let (min, max) = self.named_scalar_bounds(symbol).unwrap_or((None, None));
                        init::numeric_zero_or_min(min, max)
                    }
                    BackingClass::Str | BackingClass::Bytes => {
                        init::string_init(self.named_string_constraint(symbol).as_ref())
                    }
                    BackingClass::Unknown => v2::InitValue {
                        derivable: true,
                        value: None,
                    },
                }
            }
            SymbolKind::Enum => self.enum_default_init(symbol),
            SymbolKind::EnumSet => v2::InitValue {
                // The empty set — no bits set (typl §5.8).
                derivable: true,
                value: Some(String::new()),
            },
            SymbolKind::Struct => v2::InitValue {
                derivable: true,
                value: None,
            },
            SymbolKind::Union => v2::InitValue {
                derivable: self.union_is_derivable(symbol),
                value: None,
            },
            SymbolKind::Const => v2::InitValue {
                derivable: false,
                value: None,
            },
            // Unreachable through `resolve_type_path`, which rejects an
            // interface in type position; kept derivable as the safe default.
            SymbolKind::Interface => v2::InitValue {
                derivable: true,
                value: None,
            },
        }
    }

    /// The materialized value of a named type's own declared `= value` init
    /// (typl §5.8), or `None` when the type declares no init. A constant-valued
    /// init resolves through the const chain in the type's defining package.
    fn declared_type_init(&self, decl: &ast::TypeDef, symbol: &Symbol) -> Option<v2::InitValue> {
        let literal = decl.init_value()?.literal()?;
        let value = match literal_kind(&literal) {
            LitKind::Number { value } => value.to_decimal_string(),
            LitKind::Bool(flag) => flag.to_string(),
            LitKind::Str(text) => text,
            LitKind::ConstRef(reference) => {
                let package = self.package_handle(&symbol.package)?;
                match self.const_value_in(package, &reference)? {
                    ConstValue::Number(value) => value.to_decimal_string(),
                    ConstValue::Bool(flag) => flag.to_string(),
                    ConstValue::Text(text) => text,
                    ConstValue::Regex(_) => return None,
                }
            }
            LitKind::Regex(_) | LitKind::Malformed => return None,
        };
        Some(v2::InitValue {
            derivable: true,
            value: Some(value),
        })
    }

    /// The derived enum init (typl §5.8): the value `0` when an enum value
    /// declares it, otherwise the lowest declared value. A degenerate enum with
    /// no integer-valued members is not derivable.
    fn enum_default_init(&self, symbol: &Symbol) -> v2::InitValue {
        let Some(Definition::Enum(decl)) = self.find_definition(symbol) else {
            return v2::InitValue {
                derivable: true,
                value: None,
            };
        };
        let values: Vec<i64> = decl
            .values()
            .filter_map(
                |value| match value.value().map(|literal| literal_kind(&literal)) {
                    Some(LitKind::Number { value }) => exact_to_i64(&value),
                    _ => None,
                },
            )
            .collect();
        match values.iter().copied().min() {
            Some(_) if values.contains(&0) => v2::InitValue {
                derivable: true,
                value: Some("0".to_string()),
            },
            Some(lowest) => v2::InitValue {
                derivable: true,
                value: Some(lowest.to_string()),
            },
            None => v2::InitValue {
                derivable: false,
                value: None,
            },
        }
    }

    /// Whether a union's derived init — its first arm's init (typl §5.8) — is
    /// derivable. The first arm resolves one level: a scalar arm defers to its
    /// scalar derivability, an enum/enumset/struct/union arm is derivable.
    fn union_is_derivable(&self, symbol: &Symbol) -> bool {
        let Some(Definition::Union(decl)) = self.find_definition(symbol) else {
            return true;
        };
        let Some(first_arm) = decl.syntax().children().find_map(ast::UnionArm::cast) else {
            return false;
        };
        let Some(path) = first_arm.type_ref() else {
            return true;
        };
        if primitive_path_keyword(&path).is_some() {
            return true;
        }
        let Some(package) = self.package_handle(&symbol.package) else {
            return true;
        };
        let resolution = resolve_package(self.db, self.ws, package, self.std);
        match self.lookup_path_in(&resolution, &path) {
            Some(arm) if arm.kind == SymbolKind::Type => self.type_symbol_is_derivable(&arm),
            _ => true,
        }
    }

    /// Whether a named scalar `type` has a derivable init (typl §5.8): numeric
    /// and boolean types always do; a string/bytes type does when its bounds
    /// admit length 0 and it carries no `match` pattern; a type with a declared
    /// init always does.
    fn type_symbol_is_derivable(&self, symbol: &Symbol) -> bool {
        let Some(Definition::Type(decl)) = self.find_definition(symbol) else {
            return true;
        };
        if decl.init_value().is_some() {
            return true;
        }
        match backing_class(decl.backing()) {
            BackingClass::Boolean
            | BackingClass::Integer
            | BackingClass::Float
            | BackingClass::Unknown => true,
            BackingClass::Str | BackingClass::Bytes => self
                .named_string_constraint(symbol)
                .is_some_and(|constraint| {
                    constraint.pattern.is_none()
                        && constraint.pattern_const.is_none()
                        && constraint.len_min.unwrap_or(0) == 0
                }),
        }
    }

    /// The length bounds and `match` pattern of a named string/bytes `type`, as
    /// an IR constraint (read-only, no diagnostics), for init validation and
    /// derivation. `None` when the symbol is not a string/bytes type.
    fn named_string_constraint(&self, symbol: &Symbol) -> Option<v2::Constraint> {
        let Definition::Type(decl) = self.find_definition(symbol)? else {
            return None;
        };
        match backing_class(decl.backing()) {
            BackingClass::Str | BackingClass::Bytes => {}
            _ => return None,
        }
        let constraint = decl.constraint();
        let (len_min, len_max) = self.string_len_bounds(constraint.as_ref());
        let (pattern, pattern_const) = self.string_pattern(constraint.as_ref());
        Some(v2::Constraint {
            min: None,
            max: None,
            step: None,
            len_min: Some(len_min),
            len_max: Some(len_max),
            pattern,
            pattern_const,
        })
    }

    /// The length bounds of a string/bytes constraint, mirroring
    /// [`Checker::lower_len_scalar`] but read-only: the §4.4 default `[0..256]`
    /// when no length bound is written, a fixed `[N]` as `(N, N)`.
    fn string_len_bounds(&self, constraint: Option<&ast::Constraint>) -> (u64, u64) {
        let Some(bound) = constraint.and_then(ast::Constraint::len) else {
            return (0, 256);
        };
        let min = bound
            .min()
            .and_then(|literal| self.numeric_literal(&literal))
            .and_then(|value| exact_to_u64(&value));
        if bound.dotdot_token().is_some() {
            let max = bound
                .max()
                .and_then(|literal| self.numeric_literal(&literal))
                .and_then(|value| exact_to_u64(&value));
            (min.unwrap_or(0), max.unwrap_or(256))
        } else {
            let n = min.unwrap_or(0);
            (n, n)
        }
    }

    /// The `match` pattern of a string constraint, resolved to regex text (an
    /// inline literal or a named regex constant), read-only.
    fn string_pattern(
        &self,
        constraint: Option<&ast::Constraint>,
    ) -> (Option<String>, Option<String>) {
        let Some(match_literal) = constraint.and_then(ast::Constraint::match_pattern) else {
            return (None, None);
        };
        match literal_kind(&match_literal) {
            LitKind::Regex(text) => (Some(text), None),
            LitKind::ConstRef(name) => match self.const_regex_value(&name) {
                Some((text, canonical)) => (Some(text), Some(canonical)),
                None => (None, Some(name)),
            },
            _ => (None, None),
        }
    }

    /// Resolves a canonical IR reference (`Speed` same-package,
    /// `ridl.std.Name` cross-package) to its declaration symbol.
    fn resolve_canonical(&self, canonical: &str) -> Option<Symbol> {
        match canonical.rsplit_once('.') {
            Some((package, name)) => {
                let handle = self.package_handle(package)?;
                declared_symbols(self.db, handle).get(name).cloned()
            }
            None => declared_symbols(self.db, self.pkg).get(canonical).cloned(),
        }
    }

    // --- field types ------------------------------------------------------

    fn lower_field_type(&mut self, field_type: &ast::FieldType, map_key: bool) -> LoweredType {
        match field_type {
            ast::FieldType::Optional(optional) => {
                let mut lowered = optional
                    .field_type()
                    .map(|inner| self.lower_field_type(&inner, map_key))
                    .unwrap_or_else(|| {
                        LoweredType::plain(v2::FieldType {
                            optional: false,
                            kind: None,
                        })
                    });
                lowered.ty.optional = true;
                lowered
            }
            ast::FieldType::Path(path) => self.lower_named_type(path),
            ast::FieldType::Primitive(node) => self.lower_primitive_type(node, map_key),
            ast::FieldType::Tuple(tuple) => {
                let fields = tuple
                    .fields()
                    .map(|field| v2::TupleField {
                        name: member_name(field.name()).unwrap_or_default(),
                        r#type: field
                            .field_type()
                            .map(|inner| self.lower_field_type(&inner, false).ty),
                    })
                    .collect();
                LoweredType::plain(v2::FieldType {
                    optional: false,
                    kind: Some(v2::field_type::Kind::Tuple(v2::TupleType { fields })),
                })
            }
            ast::FieldType::Array(array) => {
                let element = array
                    .element()
                    .map(|inner| Box::new(self.lower_field_type(&inner, false).ty));
                let (min, max) = self.collection_bounds(
                    array.bound(),
                    array.syntax().text_range(),
                    DiagCode::TYPL_201,
                    "array",
                );
                LoweredType::plain(v2::FieldType {
                    optional: false,
                    kind: Some(v2::field_type::Kind::Array(Box::new(v2::ArrayType {
                        element,
                        min,
                        max,
                    }))),
                })
            }
            ast::FieldType::Map(map) => {
                let key = map.key().map(|inner| Box::new(self.lower_map_key(&inner)));
                let value = map
                    .value()
                    .map(|inner| Box::new(self.lower_field_type(&inner, false).ty));
                let (min, max) = self.collection_bounds(
                    map.bound(),
                    map.syntax().text_range(),
                    DiagCode::TYPL_202,
                    "map",
                );
                LoweredType::plain(v2::FieldType {
                    optional: false,
                    kind: Some(v2::field_type::Kind::Map(Box::new(v2::MapType {
                        key,
                        value,
                        min,
                        max,
                    }))),
                })
            }
        }
    }

    fn lower_named_type(&mut self, path: &ast::PathType) -> LoweredType {
        match self.resolve_type_path(path) {
            PathTarget::Symbol(symbol) => {
                let scalar_bounds = (symbol.kind == SymbolKind::Type)
                    .then(|| self.named_scalar_bounds(&symbol))
                    .flatten();
                // A field typed by a named string/bytes `type` carries that
                // type's length bound and `match` pattern for init validation
                // (the T14 field-init obligation; TYPL-109).
                let init_constraint = (symbol.kind == SymbolKind::Type)
                    .then(|| self.named_string_constraint(&symbol))
                    .flatten();
                LoweredType {
                    ty: v2::FieldType {
                        optional: false,
                        kind: Some(v2::field_type::Kind::Named(self.canonical_ref(&symbol))),
                    },
                    scalar_bounds,
                    init_constraint,
                }
            }
            PathTarget::Unresolved(written) => LoweredType::plain(v2::FieldType {
                optional: false,
                kind: Some(v2::field_type::Kind::Named(written)),
            }),
        }
    }

    fn lower_primitive_type(&mut self, node: &ast::PrimitiveType, map_key: bool) -> LoweredType {
        let (primitive, class) = primitive_of(node);
        let span = node.syntax().text_range();
        match node.constraint() {
            Some(constraint) => {
                // An inline constrained scalar (§5.2, §15.3): the constraint
                // nests inside the primitive in field position. The enclosing
                // Field's declared_init/init are authoritative; the nested
                // TypeDef's stay unset.
                let parts = self.lower_scalar(class, Some(constraint), span);
                let bounds = (parts.min.clone(), parts.max.clone());
                // An inline string/bytes scalar carries its length bound and
                // `match` pattern for init validation (TYPL-109); a numeric
                // inline scalar validates through `scalar_bounds`.
                let init_constraint = matches!(class, BackingClass::Str | BackingClass::Bytes)
                    .then(|| parts.constraint.clone())
                    .flatten();
                LoweredType {
                    ty: v2::FieldType {
                        optional: false,
                        kind: Some(v2::field_type::Kind::InlineScalar(Box::new(v2::TypeDef {
                            backing: Some(v2::Backing {
                                kind: Some(v2::backing::Kind::Primitive(primitive as i32)),
                            }),
                            constraint: parts.constraint,
                            declared_init: None,
                            init: None,
                            width: parts.width,
                        }))),
                    },
                    scalar_bounds: Some(bounds),
                    init_constraint,
                }
            }
            None => {
                if !map_key {
                    match class {
                        // §15.3: bare string/bytes never appear directly as a
                        // field or tuple-field type.
                        BackingClass::Str | BackingClass::Bytes => self.error(
                            DiagCode::TYPL_208,
                            span,
                            format!(
                                "`{}` must not be used directly as a field type — define a named `type`",
                                primitive_noun(class)
                            ),
                        ),
                        BackingClass::Integer => self.warning(
                            DiagCode::TYPL_101,
                            span,
                            "`integer` without a range constraint".to_string(),
                        ),
                        BackingClass::Float => self.warning(
                            DiagCode::TYPL_102,
                            span,
                            "`float` without both a range and a `step`".to_string(),
                        ),
                        BackingClass::Boolean | BackingClass::Unknown => {}
                    }
                }
                LoweredType::plain(v2::FieldType {
                    optional: false,
                    kind: Some(v2::field_type::Kind::Primitive(primitive as i32)),
                })
            }
        }
    }

    /// Lowers a map key, enforcing the §12.2 key shape: a named string type
    /// or a primitive (TYPL-209).
    fn lower_map_key(&mut self, key: &ast::FieldType) -> v2::FieldType {
        match key {
            ast::FieldType::Primitive(_) => self.lower_field_type(key, true).ty,
            ast::FieldType::Path(path) => match self.resolve_type_path(path) {
                PathTarget::Symbol(symbol) => {
                    if !(symbol.kind == SymbolKind::Type && self.type_is_string_backed(&symbol)) {
                        self.error(
                            DiagCode::TYPL_209,
                            path.syntax().text_range(),
                            format!(
                                "map key `{}` is not a named string type or a primitive",
                                symbol.name
                            ),
                        );
                    }
                    v2::FieldType {
                        optional: false,
                        kind: Some(v2::field_type::Kind::Named(self.canonical_ref(&symbol))),
                    }
                }
                PathTarget::Unresolved(written) => v2::FieldType {
                    optional: false,
                    kind: Some(v2::field_type::Kind::Named(written)),
                },
            },
            _ => {
                self.error(
                    DiagCode::TYPL_209,
                    key.syntax().text_range(),
                    "map key is not a named string type or a primitive".to_string(),
                );
                self.lower_field_type(key, true).ty
            }
        }
    }

    fn type_is_string_backed(&self, symbol: &Symbol) -> bool {
        matches!(
            self.find_definition(symbol),
            Some(Definition::Type(decl))
                if matches!(
                    decl.backing(),
                    Some(ast::Backing::Primitive(node)) if node.string_token().is_some()
                )
        )
    }

    fn collection_bounds(
        &mut self,
        bound: Option<ast::Bound>,
        span: TextRange,
        code: DiagCode,
        what: &str,
    ) -> (u64, u64) {
        let Some(bound) = bound else {
            self.error(code, span, format!("{what} without explicit bounds"));
            return (0, 0);
        };
        let min = bound
            .min()
            .and_then(|literal| self.numeric_literal(&literal))
            .and_then(|value| exact_to_u64(&value));
        if bound.dotdot_token().is_some() {
            let max = bound
                .max()
                .and_then(|literal| self.numeric_literal(&literal))
                .and_then(|value| exact_to_u64(&value));
            match max {
                Some(max) => (min.unwrap_or(0), max),
                None => {
                    // `[T; 0..]` — an open upper bound is unbounded.
                    self.error(code, span, format!("{what} without explicit bounds"));
                    (min.unwrap_or(0), 0)
                }
            }
        } else {
            let n = min.unwrap_or(0);
            (n, n)
        }
    }

    // --- enums, enum sets -------------------------------------------------

    fn lower_enum(&mut self, decl: &ast::EnumDef) -> v2::EnumDef {
        // Pre-pass over the tombstones for TYPL-210/211.
        let mut reserved_names: HashSet<String> = HashSet::new();
        let mut reserved_values: HashSet<i64> = HashSet::new();
        for entry in decl.reserved() {
            self.record_reserved(&entry, &mut reserved_names, &mut reserved_values);
        }

        let mut values = Vec::new();
        let mut reserved = Vec::new();
        let mut seen: HashSet<i64> = HashSet::new();
        // Values and tombstones interleave in source order; the typed
        // iterators are per-kind, so walk the children directly.
        for child in decl.syntax().children() {
            if let Some(entry) = ast::ReservedEntry::cast(child.clone()) {
                // The retired identity of an enum tombstone is the value, not
                // an ordinal slot (§7.4).
                reserved.push(lower_reserved(&entry, 0));
                continue;
            }
            let Some(value_node) = ast::EnumValue::cast(child) else {
                continue;
            };
            let Some(name) = member_name(value_node.name()) else {
                continue;
            };
            if reserved_names.contains(&name) {
                self.error(
                    DiagCode::TYPL_210,
                    member_name_range(value_node.name(), value_node.syntax()),
                    format!("enum value `{name}` re-declares a `reserved` name"),
                );
            }
            // RIDL-307 (warning): an `error` enum must not re-declare a
            // Stratum-2 contract-error category name — reserved vocabulary
            // (ridl §10.2). Checked wherever error enums lower, both profiles.
            if decl.is_error() && STRATUM_2_CATEGORIES.contains(&name.as_str()) {
                self.warning(
                    DiagCode::RIDL_307,
                    member_name_range(value_node.name(), value_node.syntax()),
                    format!(
                        "`error` enum value `{name}` is a reserved Stratum-2 contract-error category — choose a different name"
                    ),
                );
            }
            let literal = value_node.value();
            let value = literal
                .as_ref()
                .and_then(|literal| match literal_kind(literal) {
                    LitKind::Number { value } => exact_to_i64(&value),
                    _ => None,
                });
            let Some(value) = value else {
                // §8: all values explicitly assigned; a missing or non-integer
                // value cannot carry a wire identity, so the entry is skipped.
                self.error(
                    DiagCode::TYPL_203,
                    member_name_range(value_node.name(), value_node.syntax()),
                    format!("enum value `{name}` has no explicit integer value"),
                );
                continue;
            };
            let value_range = literal
                .as_ref()
                .map(|literal| literal.syntax().text_range())
                .unwrap_or_default();
            if reserved_values.contains(&value) {
                self.error(
                    DiagCode::TYPL_210,
                    value_range,
                    format!("enum value {value} re-uses a `reserved` value"),
                );
            }
            if !seen.insert(value) {
                self.error(
                    DiagCode::TYPL_203,
                    value_range,
                    format!("duplicate enum value {value}"),
                );
            }
            values.push(v2::EnumValue {
                name,
                value,
                doc: String::new(),
            });
        }
        v2::EnumDef { values, reserved }
    }

    fn lower_enum_set(&mut self, decl: &ast::EnumSetDef) -> v2::EnumSetDef {
        let mut backing_enum = None;
        let mut bits: Vec<v2::EnumValue> = Vec::new();

        if let Some(backing_ref) = decl.backing_ref() {
            // The derived form (§9.2): copy the backing enum's values so
            // consumers never chase the reference.
            match self.resolve_type_path(&backing_ref) {
                PathTarget::Symbol(symbol) if symbol.kind == SymbolKind::Enum => {
                    backing_enum = Some(self.canonical_ref(&symbol));
                    if let Some(Definition::Enum(enum_decl)) = self.find_definition(&symbol) {
                        for value in enum_decl.values() {
                            let Some(name) = member_name(value.name()) else {
                                continue;
                            };
                            let Some(bit) =
                                value
                                    .value()
                                    .and_then(|literal| match literal_kind(&literal) {
                                        LitKind::Number { value } => exact_to_i64(&value),
                                        _ => None,
                                    })
                            else {
                                continue;
                            };
                            bits.push(v2::EnumValue {
                                name,
                                value: bit,
                                doc: String::new(),
                            });
                        }
                    }
                }
                PathTarget::Symbol(symbol) => {
                    self.error(
                        DiagCode::NONE,
                        backing_ref.syntax().text_range(),
                        format!(
                            "expected an enum, but `{}` names {} {}",
                            symbol.name,
                            kind_article(symbol.kind),
                            kind_noun(symbol.kind)
                        ),
                    );
                    backing_enum = Some(self.canonical_ref(&symbol));
                }
                PathTarget::Unresolved(written) => backing_enum = Some(written),
            }
        } else {
            // The standalone form (§9.1).
            for bit in decl.bits() {
                let Some(name) = member_name(bit.name()) else {
                    continue;
                };
                let Some(value) = bit
                    .value()
                    .and_then(|literal| match literal_kind(&literal) {
                        LitKind::Number { value } => exact_to_i64(&value),
                        _ => None,
                    })
                else {
                    continue;
                };
                let range = bit
                    .value()
                    .map(|literal| literal.syntax().text_range())
                    .unwrap_or_else(|| member_name_range(bit.name(), bit.syntax()));
                if bits.iter().any(|existing| existing.value == value) {
                    self.error(
                        DiagCode::TYPL_207,
                        range,
                        format!("duplicate enumset bit position {value}"),
                    );
                }
                bits.push(v2::EnumValue {
                    name,
                    value,
                    doc: String::new(),
                });
            }
        }

        // `enumset_width` saturates past bit 63 with no error; the checker
        // owns the domain rejection — the language layer is int64 (§9.3).
        let mut highest = 0u32;
        for bit in &bits {
            if bit.value < 0 || bit.value > 63 {
                self.error(
                    DiagCode::TYPL_111,
                    name_range(decl),
                    format!(
                        "enumset bit position {} is outside the int64 domain (bits 0..63)",
                        bit.value
                    ),
                );
            } else {
                highest = highest.max(bit.value as u32);
            }
        }
        v2::EnumSetDef {
            backing_enum,
            bits,
            width: v2::IntWidth::from(enumset_width(highest)) as i32,
        }
    }

    // --- unions -----------------------------------------------------------

    fn lower_union(&mut self, decl: &ast::UnionDef) -> v2::UnionDef {
        let mut reserved_names: HashSet<String> = HashSet::new();
        let mut reserved_values: HashSet<i64> = HashSet::new();
        for entry in decl.reserved() {
            self.record_reserved(&entry, &mut reserved_names, &mut reserved_values);
        }

        let mut arms = Vec::new();
        let mut reserved = Vec::new();
        // (arm index, is_error) for every arm whose type resolved.
        let mut resolved_kinds: Vec<bool> = Vec::new();
        let mut all_resolved = true;
        let mut ordinal = 0u32;
        // Arms and tombstones interleave in source order and share the
        // ordinal counter (§7.4).
        for child in decl.syntax().children() {
            if let Some(entry) = ast::ReservedEntry::cast(child.clone()) {
                ordinal += 1;
                reserved.push(lower_reserved(&entry, ordinal));
                continue;
            }
            let Some(arm) = ast::UnionArm::cast(child) else {
                continue;
            };
            ordinal += 1;
            let Some(name) = member_name(arm.name()) else {
                continue;
            };
            if reserved_names.contains(&name) {
                self.error(
                    DiagCode::TYPL_210,
                    member_name_range(arm.name(), arm.syntax()),
                    format!("union arm `{name}` re-declares a `reserved` name"),
                );
            }
            let (type_ref, arm_is_error) = match arm.type_ref() {
                Some(path) => {
                    if let Some(keyword) = primitive_path_keyword(&path) {
                        // §10: arms reference named types only.
                        self.error(
                            DiagCode::TYPL_204,
                            path.syntax().text_range(),
                            format!("union arm `{name}` has primitive type `{keyword}` — arms reference named types only"),
                        );
                        all_resolved = false;
                        (keyword, None)
                    } else {
                        match self.resolve_type_path(&path) {
                            PathTarget::Symbol(symbol) => {
                                (self.canonical_ref(&symbol), Some(symbol.is_error))
                            }
                            PathTarget::Unresolved(written) => {
                                all_resolved = false;
                                (written, None)
                            }
                        }
                    }
                }
                None => {
                    all_resolved = false;
                    (String::new(), None)
                }
            };
            if let Some(is_error) = arm_is_error {
                resolved_kinds.push(is_error);
            }
            arms.push(v2::UnionArm {
                name,
                ordinal,
                type_ref,
                doc: String::new(),
            });
        }

        // Error/result analysis (§10.1–§10.2), on fully resolved unions only —
        // an unresolved arm already carries its own diagnostic.
        let mut is_result = false;
        if decl.is_error() {
            // TYPL-214: every arm of an `error union` is error-typed.
            for child in decl.syntax().children() {
                let Some(arm) = ast::UnionArm::cast(child) else {
                    continue;
                };
                let Some(path) = arm.type_ref() else { continue };
                if primitive_path_keyword(&path).is_some() {
                    continue;
                }
                let Some(symbol) = self.lookup_path(&path) else {
                    continue;
                };
                if symbol.kind == SymbolKind::Const {
                    continue;
                }
                if !symbol.is_error {
                    let name = member_name(arm.name()).unwrap_or_default();
                    self.error(
                        DiagCode::TYPL_214,
                        path.syntax().text_range(),
                        format!("`error union` arm `{name}` is not error-typed"),
                    );
                }
            }
        } else if all_resolved && !resolved_kinds.is_empty() {
            let error_arms = resolved_kinds.iter().filter(|is_error| **is_error).count();
            let success_arms = resolved_kinds.len() - error_arms;
            if error_arms > 0 && success_arms > 0 {
                // §10.2: a mix is a result union only as exactly one success
                // arm plus one error arm.
                if error_arms == 1 && success_arms == 1 {
                    is_result = true;
                } else {
                    self.error(
                        DiagCode::TYPL_213,
                        name_range(decl),
                        format!(
                            "union mixes {success_arms} success and {error_arms} error arms — a result union has exactly one of each"
                        ),
                    );
                }
            }
        }

        v2::UnionDef {
            arms,
            is_result,
            reserved,
        }
    }

    /// Records one `reserved` entry into the name/value sets, warning on a
    /// duplicate (TYPL-211). The "dangling" half of the §16.3 rule needs the
    /// previous IR snapshot and belongs to `ridl-diff` (E2.8).
    fn record_reserved(
        &mut self,
        entry: &ast::ReservedEntry,
        names: &mut HashSet<String>,
        values: &mut HashSet<i64>,
    ) {
        if let Some(name) = member_name(entry.name()) {
            if !names.insert(name.clone()) {
                self.warning(
                    DiagCode::TYPL_211,
                    entry.syntax().text_range(),
                    format!("duplicate `reserved` entry `{name}`"),
                );
            }
            return;
        }
        if let Some(value) = entry
            .literal()
            .and_then(|literal| match literal_kind(&literal) {
                LitKind::Number { value } => exact_to_i64(&value),
                _ => None,
            })
            && !values.insert(value)
        {
            self.warning(
                DiagCode::TYPL_211,
                entry.syntax().text_range(),
                format!("duplicate `reserved` entry {value}"),
            );
        }
    }

    // --- the E2.1b interaction structural pass ----------------------------
    //
    // Diagnostics only — the IR is untouched until task 6. The task 3 parser
    // over-approximates the interaction grammar deliberately; every
    // over-approximation is narrowed here so none leaks to IR lowering:
    //
    // - payloads and params parse as any `FieldType` → narrowed to the
    //   Appendix C shapes (named type; `final` also arrays; params also
    //   streams) with pointed FORM-102 messages;
    // - the `error` modifier parses on `interface` → TYPL-212;
    // - a return type parses on `command` → RIDL-104;
    // - timing parses on `command`/`query`/`final` → RIDL-106 on all three:
    //   one rule (timing belongs to `signal` and `event`, §9), one code;
    // - an attr block parses on `signal`/`event`/`final` → RIDL-106 on
    //   `final`; predicates draw RIDL-301/-302, keys the gf §4.3 allow-list
    //   (FORM-106/-107/-108);
    // - an init value parses on `event` and `final` → FORM-102;
    // - a typl declaration parses inside an interface or service body →
    //   RIDL-107, raised by the parser where the keyword is recognised;
    // - a stream `<T>` parses in every type position → RIDL-201 on
    //   signal/event payloads, FORM-102 on `final`, TYPL-301 elsewhere
    //   (struct fields and collections, ridl §12.3), via
    //   [`Checker::check_stream_positions`];
    // - timing and attrs parse in either order → no separate order rule:
    //   no interaction kind legally carries both, so every wrong-order
    //   combination already draws its kind rule.

    /// RIDL-401: an interaction re-declaring a name a `reserved` tombstone
    /// retired. The secondary label points at the tombstone, so the reader does
    /// not have to search the body for what the message is talking about.
    fn redeclared_reserved(&mut self, name: &str, range: TextRange, tombstone: TextRange) {
        self.error_with_label(
            DiagCode::RIDL_401,
            range,
            format!(
                "`{name}` is retired: this body reserves the name, and a retired name keeps its \
                 wire identity for ever, so a consumer still holding the old contract would read \
                 this interaction as the retired one. Give it a different name (ridl §11)"
            ),
            tombstone,
            format!("`{name}` is retired here"),
        );
    }

    /// RIDL-402: two interactions of one body declaring the same name. The
    /// secondary label points at the declaration that wins, because which of
    /// the two survives is the part a reader cannot guess.
    fn duplicate_interaction(&mut self, name: &str, range: TextRange, first: TextRange) {
        self.error_with_label(
            DiagCode::RIDL_402,
            range,
            format!(
                "`{name}` is already declared in this body — a name identifies one interaction, \
                 so this second declaration is dropped and takes no slot of its own. Rename it, \
                 or fold the two into one declaration (ridl §14.1)"
            ),
            first,
            format!("`{name}` is declared here, and this is the one that is kept"),
        );
    }

    /// The structural rules of one interface (ridl §16, E2 task 5).
    /// Checks one interface and lowers it to its IR shape (ridl §14.0, §11;
    /// E2.1b–c): the structural diagnostics accumulate exactly as in the
    /// E2.1b pass, and every surviving member lowers to an interaction
    /// `Decl` with its §11 ordinal — the same assignment
    /// [`Checker::lower_service_inline`] makes over a service's inline body.
    fn lower_interface(&mut self, def: &ast::InterfaceDef) -> v2::Interface {
        // TYPL-212: `error` is failure vocabulary for composites only
        // (typl §10.1) — an interface is not one.
        if def.is_error() {
            self.error(
                DiagCode::TYPL_212,
                name_range(def),
                "`error` is not valid on an `interface` declaration — only `struct`, `enum`, and `union`"
                    .to_string(),
            );
        }

        // RIDL-107 (a typl declaration inside the body) is the parser's: it
        // recognises the keyword, recovers the declaration into an ErrorNode,
        // and raises the code there. Raising it a second time here paired every
        // RIDL-107 with a contradicting FORM-102 at the same span.

        // Pre-pass: the reserved tombstone names (ridl §11). Recorded debt
        // (issue #172, M1): duplicate `reserved` tombstones in one interface
        // are silent here although each occupies an ordinal slot and shifts
        // the later ordinals — whether that deserves a diagnostic (TYPL-211
        // is the struct-side precedent) is decided in a later task.
        // The tombstone's own span is kept, not just its name: RIDL-401 points
        // the reader at the `reserved` entry that retired the name.
        let mut reserved: HashMap<String, TextRange> = HashMap::new();
        for member in def.members() {
            if let ast::InterfaceMember::Reserved(entry) = &member
                && let Some(name) = member_name(entry.name())
            {
                // `or_insert_with`, not `insert`: `HashSet::insert` kept the
                // existing entry and `HashMap::insert` overwrites it, so a
                // second tombstone for one name would move RIDL-401's label
                // onto the later `reserved` line for no reason.
                reserved
                    .entry(name)
                    .or_insert_with(|| member_name_range(entry.name(), entry.syntax()));
            }
        }

        // Pre-pass: the interface's own signals, typed for the contract
        // environment. A `require` on any interaction may read them (ridl §13),
        // including one declared later in the body, so they are gathered before
        // the members are lowered.
        self.interface_name = declared_name(def).unwrap_or_default();
        self.interface_internal = def.is_internal();
        self.interface_signals = def
            .members()
            .filter_map(|member| match member {
                ast::InterfaceMember::Signal(signal) => {
                    let name = member_name(signal.name())?;
                    let payload = signal.payload()?;
                    Some((name, self.expr_type_of_field_type(&payload)))
                }
                _ => None,
            })
            .collect();

        // The winner's span is kept so RIDL-402 can point at it.
        let mut seen: HashMap<String, TextRange> = HashMap::new();
        let mut interactions = Vec::new();
        let mut ordinal = 0u32;
        for member in def.members() {
            if !matches!(member, ast::InterfaceMember::Reserved(_)) {
                // A member the parser recovered without a name — FORM-101 (no
                // name token) or FORM-105 (a family reserved word used as one)
                // — keeps its ordinal slot but does not lower. An empty
                // `Decl.name` is not a name any backend can emit, and the
                // parser has already reported the source defect, so no second
                // diagnostic is raised here. This is the rule
                // [`Checker::lower_definition`] already applies to a nameless
                // top-level declaration.
                let Some(name) = member_name(member.name()) else {
                    ordinal += 1;
                    continue;
                };
                let range = member_name_range(member.name(), member.syntax());
                if let Some(tombstone) = reserved.get(&name).copied() {
                    self.redeclared_reserved(&name, range, tombstone);
                }
                // RIDL-402: first wins; the loser is excluded from the
                // lowering, is not re-checked, and holds no ordinal slot — the
                // same rule `lower_service_inline` applies below.
                //
                // The map is read and only then written. `HashMap::insert`
                // returns the previous value *and replaces it*, where the
                // `HashSet` this map grew out of kept the existing entry, so
                // writing unconditionally would make the third declaration of
                // one name point its label at the second — which is itself
                // dropped, while the label says it is the one that is kept.
                if let Some(first) = seen.get(&name).copied() {
                    self.duplicate_interaction(&name, range, first);
                    continue;
                }
                seen.insert(name, range);
            }
            ordinal += 1;
            interactions.push(self.lower_interaction(&member, ordinal));
        }

        self.interface_signals.clear();
        self.interface_name.clear();
        self.interface_internal = false;

        let doc_info = docs::scan(&def.doc_comments());
        let visibility = if def.is_internal() {
            v2::Visibility::Internal
        } else {
            v2::Visibility::Public
        };
        v2::Interface {
            name: declared_name(def).unwrap_or_default(),
            visibility: visibility as i32,
            doc: doc_info.doc,
            labels: doc_info.labels,
            deprecated: doc_info.deprecated,
            interactions,
        }
    }

    /// Lowers one surviving interface member to its interaction `Decl`
    /// (ridl §14.1): the doc envelope (typl §14 through the E1 scanner), the
    /// §11 ordinal, and the kind. Visibility and `is_error` stay unset on
    /// interactions.
    fn lower_interaction(&mut self, member: &ast::InterfaceMember, ordinal: u32) -> v2::Decl {
        // The interaction name is half of the observer id its contracts carry
        // (E2.5), so it is read before the member lowers.
        let interaction_name = member_name(member.name()).unwrap_or_default();
        let kind = match member {
            ast::InterfaceMember::Signal(signal) => {
                v2::decl::Kind::SignalDef(self.lower_signal(signal))
            }
            ast::InterfaceMember::Event(event) => v2::decl::Kind::EventDef(self.lower_event(event)),
            ast::InterfaceMember::Command(command) => {
                v2::decl::Kind::CommandDef(self.lower_command(command, &interaction_name))
            }
            ast::InterfaceMember::Query(query) => {
                v2::decl::Kind::QueryDef(self.lower_query(query, &interaction_name))
            }
            ast::InterfaceMember::Final(fin) => v2::decl::Kind::FinalDef(self.lower_final(fin)),
            // The tombstone stores its ordinal twice — on the `Decl`
            // envelope AND in `Reserved`. The schema cannot enforce the
            // agreement, so the lowering sets both from the one counter.
            ast::InterfaceMember::Reserved(entry) => {
                v2::decl::Kind::ReservedSlot(lower_reserved(entry, ordinal))
            }
        };
        let doc_info = docs::scan(&member.doc_comments());
        v2::Decl {
            // A tombstone's `Decl` name stays empty — the retired name lives
            // in `Reserved.name` (typl §7.4).
            name: match member {
                ast::InterfaceMember::Reserved(_) => String::new(),
                _ => member_name(member.name()).unwrap_or_default(),
            },
            visibility: v2::Visibility::Unspecified as i32,
            is_error: false,
            doc: doc_info.doc,
            labels: doc_info.labels,
            deprecated: doc_info.deprecated,
            ordinal,
            kind: Some(kind),
        }
    }

    // --- service lowering (E2.13, ridl reference §14.5) ------------------
    //
    // Kept in their own functions so a rebase against a concurrently edited
    // check.rs stays clean: `lower_service` and its two helpers reuse the
    // shared `lower_interaction`/`error` methods but add no shared code.

    /// Lowers one `service` declaration to its IR shape (ridl §14.5): a
    /// global, published declaration of an interface — either by naming a
    /// shared shape after `:` or with an inline body. A service is
    /// posture-neutral by design (§14.5): providing and requiring it are rsdl
    /// concerns (§14.6), so nothing beyond the shape lowers here. Its dotted
    /// name lives in the workspace catalog namespace, not the type namespace,
    /// so it is never a `SymbolKind`. Services always publish with public
    /// visibility — a global contract takes no `internal` modifier.
    fn lower_service(&mut self, service: &ast::ServiceDef) -> v2::Service {
        let dotted = service.name();
        let name = dotted
            .as_ref()
            .map(|dotted| significant_text(dotted.syntax()))
            .unwrap_or_default();
        if let Some(dotted) = &dotted {
            self.check_service_name(dotted);
        }
        let doc_info = docs::scan(&service.doc_comments());
        let shape = match service.interface_ref() {
            // The named-shape form: the reference must name an interface.
            Some(path) => Some(v2::service::Shape::InterfaceRef(
                self.lower_service_ref(&path),
            )),
            // The inline-shape form: an anonymous interface (`name == ""`).
            None => Some(v2::service::Shape::Inline(
                self.lower_service_inline(service),
            )),
        };
        v2::Service {
            name,
            visibility: v2::Visibility::Public as i32,
            doc: doc_info.doc,
            labels: doc_info.labels,
            deprecated: doc_info.deprecated,
            shape,
        }
    }

    /// A service's dotted global name is reverse-domain, like a package name:
    /// every segment is `lowercase_id` — an ASCII lowercase letter followed by
    /// ASCII lowercase letters or digits (ridl §14.5, ADR-0002 §1, the same
    /// rule the manifest enforces for package names). The parser accepts any
    /// identifier run and the checker narrows, the E1 discipline. No §16 code
    /// covers the name shape, so this is a description-first diagnostic (the
    /// [`Checker::resolve_type_path`] precedent).
    fn check_service_name(&mut self, dotted: &ast::DottedName) {
        for segment in dotted.segments() {
            let text = segment.text();
            if !is_lowercase_name_segment(text) {
                self.error(
                    DiagCode::NONE,
                    segment.text_range(),
                    format!(
                        "service name segment `{text}` is not lowercase — a service has a dotted, reverse-domain global name"
                    ),
                );
            }
        }
    }

    /// Resolves a service's named interface shape (ridl §14.5). The reference
    /// must name an `interface`; a type, constant, or unknown name is RIDL-141
    /// (§16.4). Returns the canonical reference for the IR — bare same-package,
    /// fully qualified cross-package — resolved through the package's imports.
    fn lower_service_ref(&mut self, path: &ast::PathType) -> String {
        let written = significant_text(path.syntax());
        let range = path.syntax().text_range();
        match self.lookup_path(path) {
            Some(symbol) if symbol.kind == SymbolKind::Interface => self.canonical_ref(&symbol),
            Some(symbol) => {
                self.error(
                    DiagCode::RIDL_141,
                    range,
                    format!(
                        "`{written}` is not an interface, and a service publishes an interface — \
                         name the interface that carries these interactions, or give the service \
                         an inline `{{ … }}` body of its own (ridl §14.5)"
                    ),
                );
                self.canonical_ref(&symbol)
            }
            None => {
                self.error(
                    DiagCode::RIDL_141,
                    range,
                    format!(
                        "`{written}` does not resolve to anything this package can see, and a \
                         service publishes an interface — declare or import the interface, or \
                         give the service an inline `{{ … }}` body of its own (ridl §14.5)"
                    ),
                );
                written
            }
        }
    }

    /// Lowers a service's inline shape to an anonymous `Interface` whose
    /// `name` is empty (ridl §14.5). Runs the same structural pass an
    /// interface body does — RIDL-401 (an interaction re-declaring a
    /// `reserved` name) and RIDL-402 (a duplicate interaction name,
    /// first-wins) — on the inline body's own §11 ordinal sequence, and
    /// RIDL-107 for a stray typl declaration inside the body.
    fn lower_service_inline(&mut self, service: &ast::ServiceDef) -> v2::Interface {
        // RIDL-107 for a stray typl declaration in this body is the parser's —
        // see [`Checker::lower_interface`].

        // An inline shape has no name of its own, so its observer stubs are
        // scoped to the service's dotted global name (E2.5). A service takes no
        // `internal` modifier (ridl §14.5), so its shape is always public.
        self.interface_internal = false;
        self.interface_name = service
            .name()
            .map(|dotted| significant_text(dotted.syntax()))
            .unwrap_or_default();
        // An inline shape IS an interface shape (ridl §14.5), so a `require`
        // on one of its interactions reads its own signals exactly as it does
        // inside an `interface` body (ridl §13) — the same pre-pass.
        self.interface_signals = service
            .inline_members()
            .filter_map(|member| match member {
                ast::InterfaceMember::Signal(signal) => {
                    let name = member_name(signal.name())?;
                    let payload = signal.payload()?;
                    Some((name, self.expr_type_of_field_type(&payload)))
                }
                _ => None,
            })
            .collect();

        let mut reserved: HashMap<String, TextRange> = HashMap::new();
        for member in service.inline_members() {
            if let ast::InterfaceMember::Reserved(entry) = &member
                && let Some(name) = member_name(entry.name())
            {
                // `or_insert_with`, not `insert`: `HashSet::insert` kept the
                // existing entry and `HashMap::insert` overwrites it, so a
                // second tombstone for one name would move RIDL-401's label
                // onto the later `reserved` line for no reason.
                reserved
                    .entry(name)
                    .or_insert_with(|| member_name_range(entry.name(), entry.syntax()));
            }
        }

        // The winner's span is kept so RIDL-402 can point at it.
        let mut seen: HashMap<String, TextRange> = HashMap::new();
        let mut interactions = Vec::new();
        let mut ordinal = 0u32;
        for member in service.inline_members() {
            if !matches!(member, ast::InterfaceMember::Reserved(_)) {
                // The nameless-member rule of the `interface` loop above,
                // kept behaviorally identical: the slot is consumed, the
                // member does not lower, and the parser owns the diagnostic.
                let Some(name) = member_name(member.name()) else {
                    ordinal += 1;
                    continue;
                };
                let range = member_name_range(member.name(), member.syntax());
                if let Some(tombstone) = reserved.get(&name).copied() {
                    self.redeclared_reserved(&name, range, tombstone);
                }
                // First-wins, read-then-write — see [`Checker::lower_interface`].
                if let Some(first) = seen.get(&name).copied() {
                    self.duplicate_interaction(&name, range, first);
                    continue;
                }
                seen.insert(name, range);
            }
            ordinal += 1;
            interactions.push(self.lower_interaction(&member, ordinal));
        }

        self.interface_signals.clear();
        self.interface_name.clear();

        v2::Interface {
            name: String::new(),
            visibility: v2::Visibility::Unspecified as i32,
            doc: String::new(),
            labels: Vec::new(),
            deprecated: None,
            interactions,
        }
    }

    /// `signal Name : type_ref init_value? timing?` (ridl §4.1, Appendix C).
    fn lower_signal(&mut self, signal: &ast::SignalDef) -> v2::SignalDef {
        let mut payload = String::new();
        let mut declared_init = None;
        let mut init = None;
        match signal.payload() {
            Some(ast::FieldType::Path(path)) => match self.resolve_type_path(&path) {
                PathTarget::Symbol(symbol) => {
                    payload = self.canonical_ref(&symbol);
                    match signal.init_value() {
                        // RIDL-110: the bare `= value` override validates
                        // against the payload constraints — the E1 scalar
                        // validation with the ridl code (§4.4) — and lowers
                        // as `declared_init` in canonical text (ADR-0008
                        // decision 2). Recorded debt (issue #172, M2): the
                        // leniency is E1's exactly — a type-mismatched
                        // literal (`= true` on a numeric payload), a value
                        // off the `step` grid, or an override on a
                        // non-`type` payload all pass silently, as they do
                        // for struct fields — although the §16.1 RIDL-110
                        // wording reads broader.
                        Some(init_value) => {
                            let parts = self.payload_scalar_parts(&symbol);
                            (declared_init, init) = self.lower_declared_init(
                                Some(init_value),
                                &parts,
                                DiagCode::RIDL_110,
                            );
                        }
                        // RIDL-109: no override, so the payload type's own
                        // init must derive (typl §5.8 through ridl §4.4);
                        // the derived value rides along either way.
                        None => {
                            let derived = self.named_type_init(&symbol);
                            if !derived.derivable {
                                self.error(
                                    DiagCode::RIDL_109,
                                    path.syntax().text_range(),
                                    format!(
                                        "signal payload type `{}` has no derivable init value and no `= value` override",
                                        symbol.name
                                    ),
                                );
                            }
                            init = Some(derived);
                        }
                    }
                }
                // Unresolved: already reported; the written text is carried
                // for honest lowering (the E1 rule).
                PathTarget::Unresolved(written) => payload = written,
            },
            Some(other) => self.error(
                DiagCode::FORM_102,
                other.syntax().text_range(),
                "signal payload must be a named type".to_string(),
            ),
            // A stream payload (RIDL-201, `check_stream_positions`) or a
            // parse error already reported.
            None => {}
        }
        self.check_member_attrs(signal.syntax(), MemberKind::Signal);
        let timing = self.resolve_member_timing(
            signal.timing(),
            member_name_range(signal.name(), signal.syntax()),
            timing::InteractionKind::Signal,
        );
        v2::SignalDef {
            payload,
            declared_init,
            init,
            timing,
        }
    }

    /// `event Name : type_ref timing?` (ridl §5.1, Appendix C).
    fn lower_event(&mut self, event: &ast::EventDef) -> v2::EventDef {
        let mut payload = String::new();
        match event.payload() {
            Some(ast::FieldType::Path(path)) => match self.resolve_type_path(&path) {
                PathTarget::Symbol(symbol) => payload = self.canonical_ref(&symbol),
                PathTarget::Unresolved(written) => payload = written,
            },
            Some(other) => self.error(
                DiagCode::FORM_102,
                other.syntax().text_range(),
                "event payload must be a named type".to_string(),
            ),
            None => {}
        }
        // Events carry no init — occurrences are not state (§4.4/§5.1); the
        // grammar has no such production, so FORM-102 with a pointed message.
        if let Some(init) = init_value_child(event.syntax()) {
            self.error(
                DiagCode::FORM_102,
                init.syntax().text_range(),
                "init value not valid on event".to_string(),
            );
        }
        self.check_member_attrs(event.syntax(), MemberKind::Event);
        let timing = self.resolve_member_timing(
            event.timing(),
            member_name_range(event.name(), event.syntax()),
            timing::InteractionKind::Event,
        );
        v2::EventDef { payload, timing }
    }

    /// Resolves a signal's or event's timing to its IR `Timing` (ridl §9,
    /// ADR-0008 decision 12): parses and validates the `@` annotation or
    /// applies the package default, accumulating the RIDL-10x diagnostics
    /// [`timing::resolve_timing`] returns. Always `Some` for a signal or event.
    fn resolve_member_timing(
        &mut self,
        annot: Option<ast::Timing>,
        anchor: TextRange,
        kind: timing::InteractionKind,
    ) -> Option<v2::Timing> {
        let file = self.file_ids[self.current_file];
        let (spec, diags) =
            timing::resolve_timing(annot.as_ref(), kind, &self.default_timing, file, anchor);
        self.diagnostics.extend(diags);
        spec.map(lower_timing_spec)
    }

    // --- the contract environment (E2 task 11) ----------------------------

    /// The contract-expression type of a declared type reference, resolved in
    /// `resolution`'s view. A declaration outside the five expr-core domains
    /// (§5.1) — a struct, a union, an enumset, a string- or bytes-backed type,
    /// a name that does not resolve — is [`ExprType::Unsupported`] carrying
    /// what it is, so a contract naming it reports the real form rather than
    /// claiming the name is unknown. Silent: each of these paths is resolved
    /// and reported by the surrounding lowering already.
    fn expr_type_of_path_in(&self, resolution: &Resolution, path: &ast::PathType) -> ExprType {
        let written = significant_text(path.syntax());
        let Some(symbol) = self.lookup_path_in(resolution, path) else {
            return ExprType::Unsupported(written);
        };
        match symbol.kind {
            SymbolKind::Enum => ExprType::EnumType(expr::qualified_ref(&symbol)),
            SymbolKind::Type => {
                let reference = expr::qualified_ref(&symbol);
                // The one duration-domain inhabitant (expr-core §5.1); its
                // `ms` backing would otherwise read as an ordinary numeric.
                if reference == "ridl.std.Duration" {
                    return ExprType::Duration;
                }
                let Some(Definition::Type(decl)) = self.find_definition(&symbol) else {
                    return ExprType::Unsupported(reference);
                };
                match backing_class(decl.backing()) {
                    BackingClass::Integer => {
                        ExprType::Numeric(reference, expr::NumericBacking::Integer)
                    }
                    BackingClass::Float => {
                        ExprType::Numeric(reference, expr::NumericBacking::Float)
                    }
                    BackingClass::Boolean => ExprType::Boolean,
                    BackingClass::Str | BackingClass::Bytes | BackingClass::Unknown => {
                        ExprType::Unsupported(reference)
                    }
                }
            }
            _ => ExprType::Unsupported(expr::qualified_ref(&symbol)),
        }
    }

    /// The same, resolved in the checked package's own view.
    fn expr_type_of_path(&self, path: &ast::PathType) -> ExprType {
        self.expr_type_of_path_in(&self.resolution, path)
    }

    fn expr_type_of_field_type(&self, field_type: &ast::FieldType) -> ExprType {
        match field_type {
            ast::FieldType::Path(path) => self.expr_type_of_path(path),
            ast::FieldType::Primitive(node) => {
                ExprType::Unsupported(significant_text(node.syntax()))
            }
            ast::FieldType::Tuple(_) => ExprType::Unsupported("a tuple".to_string()),
            ast::FieldType::Array(_) => ExprType::Unsupported("an array".to_string()),
            ast::FieldType::Map(_) => ExprType::Unsupported("a map".to_string()),
            ast::FieldType::Optional(_) => ExprType::Unsupported("an optional".to_string()),
        }
    }

    /// The typed parameter environment of an interaction (expr-core §6 item 1).
    /// Every named parameter is present: one whose type is outside the subset
    /// domains carries [`ExprType::Unsupported`], so naming it is RIDL-306 with
    /// a message about its type and not about resolution.
    fn param_expr_types(&self, params: Option<&ast::ParamList>) -> Vec<(String, ExprType)> {
        let Some(params) = params else {
            return Vec::new();
        };
        params
            .params()
            .filter_map(|param| {
                let name = member_name(param.name())?;
                let declared = match param.param_type()? {
                    ast::ParamType::Field(field_type) => self.expr_type_of_field_type(&field_type),
                    ast::ParamType::Stream(_) => ExprType::Unsupported("a stream".to_string()),
                };
                Some((name, declared))
            })
            .collect()
    }

    /// The type of `result` in an `ensure` (expr-core §6 item 2): the named
    /// return type, the named-field tuple, or the success arm of an inline
    /// fallible return — an `ensure` observes the value the query returned.
    /// A return shape outside the subset domains carries
    /// [`ExprType::Unsupported`].
    fn result_expr_type(&self, return_type: &ast::ReturnType) -> ExprType {
        if let Some(tuple) = return_type.tuple_type() {
            let fields: Vec<(String, Option<ExprType>)> = tuple
                .fields()
                .filter_map(|field| {
                    let name = member_name(field.name())?;
                    Some((
                        name,
                        field
                            .field_type()
                            .map(|found| self.expr_type_of_field_type(&found)),
                    ))
                })
                .collect();
            if fields.is_empty() {
                return ExprType::Unsupported("an empty tuple".to_string());
            }
            return ExprType::tuple(&fields);
        }
        if let Some(fallible) = return_type.fallible_type() {
            return match fallible.ok() {
                Some(ok) => self.expr_type_of_path(&ok),
                None => ExprType::Unsupported("a fallible return".to_string()),
            };
        }
        if return_type.stream_type().is_some() {
            return ExprType::Unsupported("a stream".to_string());
        }
        match return_type.type_ref() {
            Some(path) => self.expr_type_of_path(&path),
            None => ExprType::Unsupported("an unknown return type".to_string()),
        }
    }

    /// The resolved package-level vocabulary a contract may name (expr-core §6
    /// items 4 and 5): every constant with the type of its declared value, and
    /// every enum with its member list.
    ///
    /// Each declaration is resolved in **its own** package's view: a constant
    /// imported from another package names its type there, and the checked
    /// package need not import that type to compare against the constant.
    fn build_contract_vocabulary(&self) -> expr::ContractVocabulary {
        let mut vocabulary = expr::ContractVocabulary::default();
        for (bound, symbol) in &self.resolution.symbols {
            match symbol.kind {
                SymbolKind::Const => {
                    let mut visiting = HashSet::new();
                    vocabulary
                        .consts
                        .insert(bound.clone(), self.const_expr_type(symbol, &mut visiting));
                }
                SymbolKind::Enum => {
                    let Some(Definition::Enum(decl)) = self.find_definition(symbol) else {
                        continue;
                    };
                    vocabulary.enums.insert(
                        bound.clone(),
                        expr::EnumDecl {
                            reference: expr::qualified_ref(symbol),
                            members: decl
                                .values()
                                .filter_map(|value| member_name(value.name()))
                                .collect(),
                        },
                    );
                }
                _ => {}
            }
        }
        vocabulary
    }

    /// The type of a constant's value: its declared type when annotated,
    /// otherwise the type of the literal it holds. A `const = const` chain is
    /// followed to its root, guarded against cycles by `visiting`.
    fn const_expr_type(
        &self,
        symbol: &Symbol,
        visiting: &mut HashSet<(String, String)>,
    ) -> ExprType {
        let unknown = || ExprType::Unsupported("an unresolved constant".to_string());
        if !visiting.insert((symbol.package.clone(), symbol.name.clone())) {
            return unknown();
        }
        let Some(Definition::Const(decl)) = self.find_definition(symbol) else {
            return unknown();
        };
        let Some(package) = self.package_handle(&symbol.package) else {
            return unknown();
        };
        let resolution = resolve_package(self.db, self.ws, package, self.std);
        // The declared type wins — it is what makes `MAX_SPEED` a `Speed` and
        // not a bare literal (expr-core §9, typl §5.7).
        if let Some(type_ref) = decl.type_ref() {
            return self.expr_type_of_path_in(&resolution, &type_ref);
        }
        let Some(literal) = decl.value() else {
            return unknown();
        };
        match literal_kind(&literal) {
            // An unannotated numeric constant is a bare literal, which unifies
            // with any numeric operand (expr-core §5.2). Its backing follows
            // the written form, the same rule the literal itself follows.
            LitKind::Number { .. } => ExprType::Numeric(
                String::new(),
                if literal.float_number_token().is_some() {
                    expr::NumericBacking::Float
                } else {
                    expr::NumericBacking::Integer
                },
            ),
            LitKind::Bool(_) => ExprType::Boolean,
            LitKind::Str(_) => ExprType::Unsupported("a string constant".to_string()),
            LitKind::Regex(_) => ExprType::Unsupported("a regex constant".to_string()),
            LitKind::ConstRef(name) => match resolution.symbols.get(&name) {
                Some(target) if target.kind == SymbolKind::Const => {
                    self.const_expr_type(&target.clone(), visiting)
                }
                _ => unknown(),
            },
            LitKind::Malformed => unknown(),
        }
    }

    /// Type-checks and lowers the `require`/`ensure` predicates of an
    /// interaction's attribute block (ridl §13, expr-core specification).
    ///
    /// Every clause the task 5 placement rules admit is checked against the
    /// guaranteed subset ([`expr::check_contract_expr`]) — RIDL-306 for a form
    /// outside it, RIDL-305 for an `ensure` that reads no `result` — and
    /// lowers its canonical one-line rendering as `Contract.source`
    /// (ADR-0008 decision 14). The scope shape is the ridl §13 table: a
    /// `require` reads the parameters and the interface's own signals; an
    /// `ensure` reads `result` and the parameters.
    ///
    /// `allow_ensure` is false on a command, where `ensure` is RIDL-302
    /// (already reported) and the `CommandDef` proto admits `require` only;
    /// the misplaced clause lowers nothing and is not type-checked. Flag and
    /// assignment attributes carry no predicate and are skipped (their
    /// diagnostics come from [`Checker::check_member_attrs`]).
    ///
    /// Every lowered clause is also an **observer stub** (E2.5): the reads it
    /// resolves ([`expr::collect_refs`]) — signals as canonical
    /// `Interface.signalName`, parameters by name, `result` as a flag — plus
    /// `observer_id`, the handle the E5/E7 observer tooling is expected to
    /// address a single clause by.
    ///
    /// The id is `"{Interface}.{interaction}.{require|ensure}[{i}]"`, and its
    /// guarantee is precisely **positional**: `i` counts the interaction's
    /// clauses **of that kind** from 0, so appending a clause of either kind
    /// never renumbers an existing one. Removing a clause does renumber the
    /// survivors of its kind — a known limitation, recorded against ADR-0008
    /// for E5/E7 (a tombstone or explicit-index mechanism).
    fn lower_contracts(
        &mut self,
        node: &ridl_syntax::SyntaxNode,
        interaction: &str,
        allow_ensure: bool,
        params: &[(String, ExprType)],
        result: Option<ExprType>,
    ) -> Vec<v2::Contract> {
        let Some(block) = node.children().find_map(ast::AttrBlock::cast) else {
            return Vec::new();
        };
        let signals = self.interface_signals.clone();
        let interface = self.interface_name.clone();
        // One counter per clause kind — the observer id numbers within its
        // kind, never across the two.
        let mut requires = 0usize;
        let mut ensures = 0usize;
        // The package vocabulary is resolved once per package and moved out of
        // `self` for the duration of the walk, which needs `&mut self` for the
        // diagnostics.
        let vocabulary = match self.contract_vocabulary.take() {
            Some(built) => built,
            None => self.build_contract_vocabulary(),
        };
        let mut contracts = Vec::new();
        for attribute in block.attributes() {
            let Some(predicate) = attribute.predicate_kind() else {
                continue;
            };
            let kind = match predicate {
                ast::PredicateKind::Require => v2::ContractKind::Require,
                ast::PredicateKind::Ensure if allow_ensure => v2::ContractKind::Ensure,
                ast::PredicateKind::Ensure => continue,
            };
            // An empty predicate is a parse error; nothing is lowered for it.
            let Some(clause) = attribute.expr() else {
                continue;
            };
            let scope = match predicate {
                ast::PredicateKind::Require => ContractScope {
                    params,
                    result: None,
                    signals: &signals,
                    vocabulary: &vocabulary,
                    resolution: &self.resolution,
                },
                ast::PredicateKind::Ensure => ContractScope {
                    params,
                    result: result.clone(),
                    signals: &[],
                    vocabulary: &vocabulary,
                    resolution: &self.resolution,
                },
            };
            let (_, diagnostics) = expr::check_contract_expr(&clause, &scope);
            let refs = expr::collect_refs(&clause, &scope);
            // The subset checker sees one expression and not its file, so the
            // file id is stamped here.
            let file = self.file_ids[self.current_file];
            self.diagnostics
                .extend(diagnostics.into_iter().map(|mut diagnostic| {
                    diagnostic.primary.file = file;
                    diagnostic
                }));
            self.check_clause_exposure(&clause, &refs);
            // `kind` is one of the two predicates matched above, never
            // `Unspecified`.
            let (kind_text, index) = match kind {
                v2::ContractKind::Ensure => ("ensure", &mut ensures),
                _ => ("require", &mut requires),
            };
            let observer_id = format!("{interface}.{interaction}.{kind_text}[{index}]");
            *index += 1;
            contracts.push(v2::Contract {
                kind: kind as i32,
                source: expr::canonical_expr_text(&clause),
                signal_refs: refs
                    .signals
                    .iter()
                    .map(|signal| format!("{interface}.{signal}"))
                    .collect(),
                param_refs: refs.params,
                uses_result: refs.uses_result,
                observer_id,
            });
        }
        self.contract_vocabulary = Some(vocabulary);
        contracts
    }

    /// TYPL-005 over one contract clause: the fourth exposure family, split off
    /// from [`Checker::report_exposures`] because it is the one that cannot be
    /// decided from the syntax alone.
    ///
    /// A clause is published verbatim — IR v2 carries its canonical source text
    /// (ADR-0008 decision 14) and both backends emit that text as data — so a
    /// package declaration it names that an importer cannot resolve leaks
    /// exactly as a payload type does. Which names reach the package vocabulary
    /// at all is a scope question: parameters, `result` and the interface's own
    /// signals bind ahead of it, and the two clause kinds do not bind the same
    /// set (an `ensure` sees no signals, ridl §13). `refs` is therefore taken
    /// from [`expr::collect_refs`] against the very scope
    /// [`expr::check_contract_expr`] was just run with, rather than
    /// re-derived: the binding order has exactly one implementation, and this
    /// reads its answer.
    fn check_clause_exposure(&mut self, clause: &ast::Expr, refs: &expr::ExprRefs) {
        // An `internal` interface's clauses expose nothing; an inline service
        // shape is always public (ridl §14.5), which is what
        // `interface_internal` records for it.
        if self.interface_internal {
            return;
        }
        let decl_name = self.interface_name.clone();
        // A constant is a read; an enum type is named as the head of an
        // `Enum.MEMBER` access. Nothing else in the guaranteed subset names a
        // package declaration (expr-core §6).
        let named: Vec<(&str, &String)> = refs
            .consts
            .iter()
            .map(|name| ("constant", name))
            .chain(refs.enum_types.iter().map(|name| ("type", name)))
            .collect();
        let exposed: Vec<(&'static str, String, TextRange)> = named
            .into_iter()
            .filter_map(|(noun, name)| {
                let symbol = self.resolution.symbols.get(name)?;
                if !symbol.internal || symbol.package != self.package_name {
                    return None;
                }
                let noun = match symbol.kind {
                    SymbolKind::Const => "constant",
                    _ => noun,
                };
                Some((noun, symbol.name.clone(), clause_ref_range(clause, name)?))
            })
            .collect();
        for (noun, name, range) in exposed {
            self.report_exposure(DiagCode::TYPL_005, range, noun, &name, &decl_name);
        }
    }

    /// `command Name '(' params ')' attr_block?` (ridl §6.1, Appendix C).
    fn lower_command(&mut self, command: &ast::CommandDef, name: &str) -> v2::CommandDef {
        // RIDL-104: a command always returns `()` — the erroneous return
        // shape is reported and not lowered (a `CommandDef` has no return
        // field to carry it).
        if let Some(return_type) = command.return_type() {
            self.error(
                DiagCode::RIDL_104,
                return_type.syntax().text_range(),
                "return type on command — a command always returns `()`; use `query` for a result"
                    .to_string(),
            );
        }
        if let Some(timing) = command.timing() {
            self.reject_timing(&timing, "command");
        }
        let params = command
            .params()
            .map(|params| self.lower_params(&params, "command"))
            .unwrap_or_default();
        self.check_member_attrs(command.syntax(), MemberKind::Command);
        let param_types = self.param_expr_types(command.params().as_ref());
        let contracts = self.lower_contracts(command.syntax(), name, false, &param_types, None);
        v2::CommandDef { params, contracts }
    }

    /// `query Name '(' params ')' ':' return_type attr_block?` (ridl §7.1,
    /// Appendix C; inline `T | E` per general form §6.1, ADR-0008 decision 1).
    fn lower_query(&mut self, query: &ast::QueryDef, name: &str) -> v2::QueryDef {
        if let Some(timing) = query.timing() {
            self.reject_timing(&timing, "query");
        }
        let params = query
            .params()
            .map(|params| self.lower_params(&params, "query"))
            .unwrap_or_default();
        let return_type = query
            .return_type()
            .and_then(|return_type| self.lower_query_return(&return_type));
        self.check_member_attrs(query.syntax(), MemberKind::Query);
        let param_types = self.param_expr_types(query.params().as_ref());
        // A query always has a `result` in an `ensure`; a return shape outside
        // the subset carries its own unsupported reason.
        let result_type = Some(match query.return_type() {
            Some(declared) => self.result_expr_type(&declared),
            None => ExprType::Unsupported("an absent return type".to_string()),
        });
        let contracts = self.lower_contracts(query.syntax(), name, true, &param_types, result_type);
        v2::QueryDef {
            params,
            return_type,
            contracts,
        }
    }

    /// The four return shapes (ridl §7): named type, named-field tuple, and
    /// stream lower as `ReturnType.value`; the inline fallible `T | E`
    /// lowers as `ReturnType.fallible`. An empty tuple is RIDL-105 and
    /// lowers nothing — a query returning `()` has no representable return.
    fn lower_query_return(&mut self, return_type: &ast::ReturnType) -> Option<v2::ReturnType> {
        let kind = if let Some(tuple) = return_type.tuple_type() {
            if tuple.fields().next().is_none() {
                self.error(
                    DiagCode::RIDL_105,
                    return_type.syntax().text_range(),
                    "query returning `()` — a query must return a value; use `command`".to_string(),
                );
                return None;
            }
            let lowered = self.lower_field_type(&ast::FieldType::Tuple(tuple), false);
            v2::return_type::Kind::Value(lowered.ty)
        } else if let Some(stream) = return_type.stream_type() {
            v2::return_type::Kind::Value(self.lower_stream(&stream))
        } else if let Some(fallible) = return_type.fallible_type() {
            // gf §6.1: the left arm is a non-error success type; the right arm
            // is exactly one error type. An error-typed left arm is the
            // arm-order mistake — the dominant diagnostic, so the right arm is
            // not separately reported; otherwise a non-error right arm draws
            // the "not an error type" message. Both are RIDL-303 (a fallible
            // return with no success path). The arms lower canonically either
            // way (honest lowering); the transport identity is derived, never
            // stored (ADR-0008 decision 4).
            let ok_path = fallible.ok();
            let err_path = fallible.err();
            let (ok, ok_symbol) = match &ok_path {
                Some(path) => self.resolve_arm(path),
                None => (String::new(), None),
            };
            let (err, err_symbol) = match &err_path {
                Some(path) => self.resolve_arm(path),
                None => (String::new(), None),
            };
            if let (Some(path), Some(symbol)) = (&ok_path, &ok_symbol)
                && symbol.is_error
            {
                self.error(
                    DiagCode::RIDL_303,
                    path.syntax().text_range(),
                    format!(
                        "success arm `{}` is an error type — write `T | E` with the error arm second",
                        symbol.name
                    ),
                );
            } else if let (Some(path), Some(symbol)) = (&err_path, &err_symbol)
                && !symbol.is_error
            {
                self.error(
                    DiagCode::RIDL_303,
                    path.syntax().text_range(),
                    format!(
                        "`{}` is not an error type — compose failure kinds into an error union first",
                        symbol.name
                    ),
                );
            }
            v2::return_type::Kind::Fallible(v2::FallibleType { ok, err })
        } else if let Some(path) = return_type.type_ref() {
            // RIDL-303: a bare error type in return position has no success
            // path (ridl §10.1) — a bare error `enum`/`struct`/`union` or a
            // named `error union` alike. A named *result* union is not an
            // error type (it is `is_result`), so it lowers here legally; its
            // canonical-form lint (RIDL-308) is task 19, not an error.
            let (named, symbol) = self.resolve_arm(&path);
            if let Some(symbol) = &symbol
                && symbol.is_error
            {
                self.error(
                    DiagCode::RIDL_303,
                    path.syntax().text_range(),
                    format!(
                        "query returns a bare error type `{}` — a query with no success path is not a query",
                        symbol.name
                    ),
                );
            }
            v2::return_type::Kind::Value(v2::FieldType {
                optional: false,
                kind: Some(v2::field_type::Kind::Named(named)),
            })
        } else {
            return None;
        };
        Some(v2::ReturnType { kind: Some(kind) })
    }

    /// Resolves a type reference to its canonical IR string: `pkg.Name` for
    /// a cross-package reference, the bare name for a same-package one, and
    /// the written text when unresolved (already reported; honest lowering).
    fn lower_type_ref(&mut self, path: &ast::PathType) -> String {
        match self.resolve_type_path(path) {
            PathTarget::Symbol(symbol) => self.canonical_ref(&symbol),
            PathTarget::Unresolved(written) => written,
        }
    }

    /// Resolves a fallible-return arm or a parameter type to its canonical IR
    /// string plus the resolved symbol — the symbol carries the `is_error`
    /// flag the arm rules (RIDL-303) and the parameter rule (RIDL-304) read.
    /// An unresolved reference carries its written text (already reported).
    fn resolve_arm(&mut self, path: &ast::PathType) -> (String, Option<Symbol>) {
        match self.resolve_type_path(path) {
            PathTarget::Symbol(symbol) => (self.canonical_ref(&symbol), Some(symbol)),
            PathTarget::Unresolved(written) => (written, None),
        }
    }

    /// Whether `symbol` names a typl **result union** — a non-error union with
    /// exactly one error arm and one success arm (typl §10.2). Recomputed from
    /// the union's arms here because the `is_result` flag lives only in the
    /// lowered IR; each arm resolves in the union's own defining package
    /// (mirrors [`Checker::lower_union`]). An unresolved or primitive arm makes
    /// the union not a clean result union.
    pub(crate) fn union_is_result(&self, symbol: &Symbol) -> bool {
        if symbol.kind != SymbolKind::Union {
            return false;
        }
        let Some(Definition::Union(decl)) = self.find_definition(symbol) else {
            return false;
        };
        if decl.is_error() {
            return false;
        }
        let Some(package) = self.package_handle(&symbol.package) else {
            return false;
        };
        let resolution = resolve_package(self.db, self.ws, package, self.std);
        let mut error_arms = 0usize;
        let mut success_arms = 0usize;
        for child in decl.syntax().children() {
            let Some(arm) = ast::UnionArm::cast(child) else {
                continue;
            };
            let Some(path) = arm.type_ref() else {
                return false;
            };
            if primitive_path_keyword(&path).is_some() {
                return false;
            }
            match self.lookup_path_in(&resolution, &path) {
                Some(arm_symbol) if arm_symbol.is_error => error_arms += 1,
                Some(_) => success_arms += 1,
                None => return false,
            }
        }
        error_arms == 1 && success_arms == 1
    }

    /// RIDL-106: a timing annotation on a kind that carries none.
    ///
    /// Timing belongs to `signal` and `event` (ridl §9); the grammar accepts
    /// `@` on all five kinds so that the narrowing is a semantic rule with a
    /// semantic message, and this is that rule for the three kinds that carry
    /// no timing. `command` and `query` used to draw FORM-102 here while
    /// `final` drew RIDL-106 — one rule under two codes, one of them a parse
    /// code whose catalogue meaning is "unexpected token", for a token the
    /// parser deliberately accepts.
    fn reject_timing(&mut self, timing: &ast::Timing, kind: &str) {
        let because = match kind {
            "command" => {
                "a command is invoked on demand, not published on a schedule, and its \
                 acknowledgement is not a publication"
            }
            "query" => {
                "a query is answered on demand, not published on a schedule, and its reply is \
                 not a publication"
            }
            _ => {
                "a `final` is provisioned externally and never republished, so it has no rate \
                 floor and no staleness bound"
            }
        };
        self.error(
            DiagCode::RIDL_106,
            timing.syntax().text_range(),
            format!(
                "a timing annotation is not valid on `{kind}` — {because}. Timing belongs to \
                 `signal` and `event` (ridl §9)"
            ),
        );
    }

    /// `final Name : (type_ref | array_type)` (ridl §8, Appendix C) — no
    /// init, no timing, no attribute block (RIDL-106).
    fn lower_final(&mut self, fin: &ast::FinalDef) -> v2::FinalDef {
        let payload = match fin.payload() {
            Some(ast::FieldType::Path(path)) => Some(v2::FieldType {
                optional: false,
                kind: Some(v2::field_type::Kind::Named(self.lower_type_ref(&path))),
            }),
            // An array payload lowers through the E1 field-type path, which
            // resolves the element and validates the bounds (typl §12.1).
            Some(array @ ast::FieldType::Array(_)) => Some(self.lower_field_type(&array, false).ty),
            Some(other) => {
                self.error(
                    DiagCode::FORM_102,
                    other.syntax().text_range(),
                    "final payload must be a named type or an array".to_string(),
                );
                None
            }
            // A stream payload (FORM-102, `check_stream_positions`) or a
            // parse error already reported.
            None => None,
        };
        if let Some(init) = init_value_child(fin.syntax()) {
            self.error(
                DiagCode::FORM_102,
                init.syntax().text_range(),
                "init value not valid on final".to_string(),
            );
        }
        if let Some(timing) = fin.timing() {
            self.reject_timing(&timing, "final");
        }
        if let Some(block) = fin.attr_block() {
            self.error(
                DiagCode::RIDL_106,
                block.syntax().text_range(),
                "an attribute block is not valid on `final` — a `final` has no timing to \
                 override and no contract to state, because it is provisioned externally and \
                 never changes while the software runs (ridl §8)"
                    .to_string(),
            );
        }
        self.check_member_attrs(fin.syntax(), MemberKind::Final);
        v2::FinalDef { payload }
    }

    /// `param_type = type_ref | stream_type` (ridl Appendix C) — `noun`
    /// names the callable kind in the FORM-102 message. A parameter whose
    /// shape is rejected lowers with no type (already reported).
    fn lower_params(&mut self, params: &ast::ParamList, noun: &str) -> Vec<v2::Param> {
        params
            .params()
            .map(|param| {
                let name = member_name(param.name()).unwrap_or_default();
                let r#type = match param.param_type() {
                    Some(ast::ParamType::Field(ast::FieldType::Path(path))) => {
                        let (canonical, symbol) = self.resolve_arm(&path);
                        // RIDL-304 (warning): an error-typed or result-union
                        // parameter sends failure toward a provider — the
                        // wrong direction for the failure channel (ridl §10.1,
                        // §16.3).
                        if let Some(symbol) = &symbol {
                            if symbol.is_error {
                                self.warning(
                                    DiagCode::RIDL_304,
                                    path.syntax().text_range(),
                                    format!(
                                        "{noun} parameter `{name}` has error type `{}` — failure flowing toward a provider",
                                        symbol.name
                                    ),
                                );
                            } else if self.union_is_result(symbol) {
                                self.warning(
                                    DiagCode::RIDL_304,
                                    path.syntax().text_range(),
                                    format!(
                                        "{noun} parameter `{name}` has result-union type `{}` — failure flowing toward a provider",
                                        symbol.name
                                    ),
                                );
                            }
                        }
                        Some(v2::FieldType {
                            optional: false,
                            kind: Some(v2::field_type::Kind::Named(canonical)),
                        })
                    }
                    Some(ast::ParamType::Field(other)) => {
                        self.error(
                            DiagCode::FORM_102,
                            other.syntax().text_range(),
                            format!("{noun} parameter must be a named type or a stream"),
                        );
                        None
                    }
                    Some(ast::ParamType::Stream(stream)) => Some(self.lower_stream(&stream)),
                    None => None,
                };
                v2::Param { name, r#type }
            })
            .collect()
    }

    /// Lowers a stream `<T>` to `FieldType.stream` (ridl §12). RIDL-202: the
    /// element is a named type or raw `string`/`bytes` (§12.2); a primitive
    /// keyword element parses as a path, is reported, and lowers its written
    /// text (honest lowering).
    fn lower_stream(&mut self, stream: &ast::StreamType) -> v2::FieldType {
        let element = if let Some(path) = stream.element_type() {
            if primitive_path_keyword(&path).is_some() {
                self.error(
                    DiagCode::RIDL_202,
                    path.syntax().text_range(),
                    format!(
                        "`{}` is a primitive, and a stream element is a named type or the raw \
                         `string`/`bytes` spelling — declare a `type` for it, so the element \
                         carries the range, unit and bounds one element promises (ridl §12.2)",
                        significant_text(path.syntax()),
                    ),
                );
                Some(v2::stream_type::Element::Named(significant_text(
                    path.syntax(),
                )))
            } else {
                Some(v2::stream_type::Element::Named(self.lower_type_ref(&path)))
            }
        } else if stream.string_token().is_some() {
            Some(v2::stream_type::Element::Primitive(
                v2::PrimitiveType::String as i32,
            ))
        } else if stream.bytes_token().is_some() {
            Some(v2::stream_type::Element::Primitive(
                v2::PrimitiveType::Bytes as i32,
            ))
        } else {
            // A missing element was a parse error.
            None
        };
        v2::FieldType {
            optional: false,
            kind: Some(v2::field_type::Kind::Stream(v2::StreamType { element })),
        }
    }

    /// The gf §4.3 attribute allow-list plus the ridl §13 predicate table,
    /// over the member's attr block (if any). In E2 the only consumable
    /// attributes are the `require`/`ensure` predicates: every flag or
    /// assignment key draws FORM-107 when gf §4.3 knows it, FORM-106 when it
    /// is unknown; a repeated key draws FORM-108.
    ///
    /// Recorded deferral (gf §4.5): attribute keys are doctrine-bound to
    /// become contextual (recognised only inside `[ ]`, not registry words).
    /// E2 does not implement that downgrade: `init` stays family-reserved —
    /// `[ init = X ]` already draws FORM-105 at parse, and signal init is the
    /// bare `= value` form (ADR-0008 decision 2) — so no key is special-cased
    /// here.
    fn check_member_attrs(&mut self, member: &ridl_syntax::SyntaxNode, kind: MemberKind) {
        let Some(block) = member.children().find_map(ast::AttrBlock::cast) else {
            return;
        };
        let noun = kind.noun();
        let mut seen: HashSet<String> = HashSet::new();
        for attribute in block.attributes() {
            match attribute.predicate_kind() {
                Some(ast::PredicateKind::Require) => {
                    if !matches!(kind, MemberKind::Command | MemberKind::Query) {
                        self.error(
                            DiagCode::RIDL_301,
                            attribute.syntax().text_range(),
                            format!(
                                "`require` not valid on {noun} — contracts belong to `command` and `query`"
                            ),
                        );
                    }
                }
                Some(ast::PredicateKind::Ensure) => match kind {
                    MemberKind::Query => {}
                    MemberKind::Command => self.error(
                        DiagCode::RIDL_302,
                        attribute.syntax().text_range(),
                        "`ensure` not valid on command — a command has no result to observe"
                            .to_string(),
                    ),
                    _ => self.error(
                        DiagCode::RIDL_301,
                        attribute.syntax().text_range(),
                        format!("`ensure` not valid on {noun} — contracts belong to `query`"),
                    ),
                },
                None => {
                    // A reserved-word key already drew FORM-105 at parse and
                    // carries no name node.
                    let Some(key) = member_name(attribute.key()) else {
                        continue;
                    };
                    let range = attribute.syntax().text_range();
                    if GF_ATTRIBUTE_KEYS.contains(&key.as_str()) {
                        self.error(
                            DiagCode::FORM_107,
                            range,
                            format!("attribute `{key}` not valid on {noun}"),
                        );
                    } else {
                        self.error(
                            DiagCode::FORM_106,
                            range,
                            format!("unknown attribute key `{key}`"),
                        );
                    }
                    if !seen.insert(key.clone()) {
                        self.error(
                            DiagCode::FORM_108,
                            range,
                            format!("duplicate attribute key `{key}`"),
                        );
                    }
                }
            }
        }
    }

    /// Classifies every stream `<T>` in a ridl-profile file by position
    /// (ridl §12.3): legal in interaction position (params and returns —
    /// a command return is already RIDL-104 wholesale), RIDL-201 on
    /// signal/event payloads, FORM-102 on `final`, TYPL-301 anywhere else
    /// (struct fields and collections — the task 3 parser hand-off).
    fn check_stream_positions(&mut self, source: &ast::SourceFile) {
        for node in source.syntax().descendants() {
            if node.kind() != SyntaxKind::StreamType {
                continue;
            }
            let range = node.text_range();
            match node.parent().map(|parent| parent.kind()) {
                Some(SyntaxKind::Param | SyntaxKind::ReturnType) => {}
                Some(SyntaxKind::SignalDef) => self.error(
                    DiagCode::RIDL_201,
                    range,
                    "a stream `<T>` is not valid on a signal payload — a signal already is the \
                     stream for state, publishing its current value on the bounds it declares. \
                     A stream is for an RPC-scoped transfer with a beginning and an end, so it \
                     belongs on a `command` parameter or a `query` return (ridl §12.3)"
                        .to_string(),
                ),
                Some(SyntaxKind::EventDef) => self.error(
                    DiagCode::RIDL_201,
                    range,
                    "a stream `<T>` is not valid on an event payload — an unbounded push of \
                     occurrences is what the event itself is. A stream is for an RPC-scoped \
                     transfer with a beginning and an end, so it belongs on a `command` \
                     parameter or a `query` return (ridl §12.3)"
                        .to_string(),
                ),
                Some(SyntaxKind::FinalDef) => self.error(
                    DiagCode::FORM_102,
                    range,
                    "stream `<T>` not valid on final".to_string(),
                ),
                _ => self.error(
                    DiagCode::TYPL_301,
                    range,
                    "stream type `<T>` not valid on a struct field or collection — interaction parameters and query returns only"
                        .to_string(),
                ),
            }
        }
    }

    /// The scalar bounds and string constraint of a signal payload's named
    /// type, for validating the `= value` override (RIDL-110) — exactly the
    /// parts a struct field typed by the same name would validate against.
    fn payload_scalar_parts(&self, symbol: &Symbol) -> ScalarParts {
        if symbol.kind != SymbolKind::Type {
            return ScalarParts::empty();
        }
        let (min, max) = self.named_scalar_bounds(symbol).unwrap_or((None, None));
        ScalarParts {
            constraint: self.named_string_constraint(symbol),
            width: None,
            min,
            max,
        }
    }

    // --- fixed-width analysis ---------------------------------------------

    /// Whether a field type is fixed-width in the given package view.
    /// `visiting` breaks reference cycles (a recursive shape is TYPL-206
    /// elsewhere; here it is simply not fixed).
    ///
    /// A string- or bytes-backed field is never fixed, even with a fixed length
    /// bound: the Rust backend realizes those backings as `String` and
    /// `Vec<u8>` (not `[u8; N]`), which carry no `#[repr(C)]` ABI, and the C
    /// header has no fixed C type for them. Excluding them keeps `fixed_layout`
    /// an honest promise — a fixed-layout struct really is a clean C ABI (C2).
    fn field_type_is_fixed(
        &self,
        resolution: &Resolution,
        field_type: &ast::FieldType,
        visiting: &mut HashSet<(String, String)>,
    ) -> bool {
        match field_type {
            ast::FieldType::Optional(_) | ast::FieldType::Map(_) => false,
            ast::FieldType::Primitive(node) => {
                let (_, class) = primitive_of(node);
                match class {
                    BackingClass::Boolean | BackingClass::Integer | BackingClass::Float => true,
                    BackingClass::Str | BackingClass::Bytes | BackingClass::Unknown => false,
                }
            }
            ast::FieldType::Path(path) => match self.lookup_path_in(resolution, path) {
                Some(symbol) => self.symbol_is_fixed(&symbol, visiting),
                None => false,
            },
            ast::FieldType::Tuple(tuple) => tuple.fields().all(|field| {
                field
                    .field_type()
                    .is_some_and(|inner| self.field_type_is_fixed(resolution, &inner, visiting))
            }),
            ast::FieldType::Array(array) => {
                array
                    .bound()
                    .is_some_and(|bound| self.bound_is_fixed(&bound))
                    && array.element().is_some_and(|element| {
                        self.field_type_is_fixed(resolution, &element, visiting)
                    })
            }
        }
    }

    fn bound_is_fixed(&self, bound: &ast::Bound) -> bool {
        if bound.dotdot_token().is_none() {
            return bound.min().is_some();
        }
        let min = bound
            .min()
            .and_then(|literal| self.numeric_literal(&literal));
        let max = bound
            .max()
            .and_then(|literal| self.numeric_literal(&literal));
        matches!((min, max), (Some(min), Some(max)) if min == max)
    }

    fn symbol_is_fixed(&self, symbol: &Symbol, visiting: &mut HashSet<(String, String)>) -> bool {
        let key = (symbol.package.clone(), symbol.name.clone());
        if !visiting.insert(key.clone()) {
            return false;
        }
        let result = match symbol.kind {
            SymbolKind::Enum | SymbolKind::EnumSet => true,
            SymbolKind::Union | SymbolKind::Const | SymbolKind::Interface => false,
            SymbolKind::Type => match self.find_definition(symbol) {
                Some(Definition::Type(decl)) => match decl.backing() {
                    Some(ast::Backing::Unit(_)) => true,
                    Some(ast::Backing::Primitive(node)) => {
                        let (_, class) = primitive_of(&node);
                        match class {
                            BackingClass::Boolean | BackingClass::Integer | BackingClass::Float => {
                                true
                            }
                            // A string/bytes-backed named type is never fixed,
                            // even with a fixed length bound — see
                            // `field_type_is_fixed` (C2).
                            BackingClass::Str | BackingClass::Bytes | BackingClass::Unknown => {
                                false
                            }
                        }
                    }
                    None => false,
                },
                _ => false,
            },
            SymbolKind::Struct => match (
                self.find_definition(symbol),
                self.package_handle(&symbol.package),
            ) {
                (Some(Definition::Struct(decl)), Some(package)) => {
                    let resolution = resolve_package(self.db, self.ws, package, self.std);
                    decl.members().all(|member| match member {
                        ast::StructMember::Reserved(_) => false,
                        ast::StructMember::Field(field) => match field.field_type() {
                            Some(ast::FieldType::Optional(_)) | None => false,
                            Some(inner) => self.field_type_is_fixed(&resolution, &inner, visiting),
                        },
                    })
                }
                _ => false,
            },
        };
        visiting.remove(&key);
        result
    }

    // --- recursion (TYPL-206) ---------------------------------------------

    /// DFS over the composite reference graph: every struct/union of the
    /// checked package that can reach itself — directly or through any chain
    /// of composite references, across packages — is TYPL-206 (§7.3).
    ///
    /// Cross-package scope, on record: the resolver's TYPL-004 walks import
    /// edges only, so a qualified-ref-only cycle (`a` holds
    /// `struct S { x: b.T }`, `b` holds `struct T { y: a.S }`, no imports)
    /// escapes it — this DFS follows qualified references and closes exactly
    /// that case. A qualified-ref-only mutual reference between packages
    /// through *non-composite* declarations (a const in `a` typed by a type
    /// of `b` and the reverse) stays permitted, as accepted: range bounds
    /// cannot take qualified references (a bound literal is a single
    /// identifier), so such a shape carries no unbounded wire size and no
    /// resolution ambiguity, and no diagnostic covers it.
    fn check_recursion(&mut self, starts: &[(String, usize, TextRange)]) {
        for (name, file_index, range) in starts {
            let start = (self.package_name.clone(), name.clone());
            let mut path = vec![start.clone()];
            let mut visited = HashSet::new();
            if self.reaches_start(&start, &start, &mut visited, &mut path) {
                let chain = path
                    .iter()
                    .skip(1)
                    .map(|(package, name)| {
                        if *package == self.package_name {
                            name.clone()
                        } else {
                            format!("{package}.{name}")
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("` -> `");
                self.current_file = *file_index;
                self.error(
                    DiagCode::TYPL_206,
                    *range,
                    format!("recursive composite reference: `{name}` -> `{chain}`"),
                );
            }
        }
    }

    /// Whether any composite edge from `node` leads back to `start`. `path`
    /// carries the cycle for the diagnostic.
    fn reaches_start(
        &self,
        node: &(String, String),
        start: &(String, String),
        visited: &mut HashSet<(String, String)>,
        path: &mut Vec<(String, String)>,
    ) -> bool {
        for edge in self.composite_edges(node) {
            path.push(edge.clone());
            if edge == *start {
                return true;
            }
            if visited.insert(edge.clone()) && self.reaches_start(&edge, start, visited, path) {
                return true;
            }
            path.pop();
        }
        false
    }

    /// The composite-typed references in the body of `(package, name)`,
    /// resolved in that package's own view.
    fn composite_edges(&self, node: &(String, String)) -> Vec<(String, String)> {
        let (package_name, decl_name) = node;
        let Some(package) = self.package_handle(package_name) else {
            return Vec::new();
        };
        let resolution = resolve_package(self.db, self.ws, package, self.std);
        let Some(symbol) = resolution.symbols.get(decl_name) else {
            return Vec::new();
        };
        let definition = match self.find_definition(symbol) {
            Some(definition @ (Definition::Struct(_) | Definition::Union(_))) => definition,
            _ => return Vec::new(),
        };
        let mut edges = Vec::new();
        for descendant in definition.syntax().descendants() {
            let Some(path) = ast::PathType::cast(descendant) else {
                continue;
            };
            let Some(target) = self.lookup_path_in(&resolution, &path) else {
                continue;
            };
            if matches!(target.kind, SymbolKind::Struct | SymbolKind::Union) {
                edges.push((target.package, target.name));
            }
        }
        edges
    }
}

// --- free helpers ---------------------------------------------------------

/// The ECMA-262 body of a typl regex literal — its text without the enclosing
/// `/…/` delimiters (typl §2.7).
fn regex_body(raw: &str) -> &str {
    raw.strip_prefix('/')
        .and_then(|rest| rest.strip_suffix('/'))
        .unwrap_or(raw)
}

/// Whether a blank line separates `definition`'s doc comment from the
/// definition (TYPL-404). Only meaningful when a doc comment is attached; the
/// check reads the whitespace token immediately before the definition — two or
/// more newlines is a blank line. Doc comments are trivia, so the AST attaches
/// them across the blank line even though the spec warns about the gap.
fn blank_line_before_definition(definition: &Definition) -> bool {
    if definition.doc_comments().is_empty() {
        return false;
    }
    matches!(
        definition.syntax().prev_sibling_or_token(),
        Some(NodeOrToken::Token(token))
            if token.kind() == SyntaxKind::Whitespace
                && token.text().matches('\n').count() >= 2
    )
}

/// The declared name of a body member (field, arm, enum value, bit,
/// tombstone) — these nodes carry a `name()` accessor but not the `HasName`
/// trait.
/// One segment of a dotted global name: an ASCII lowercase letter followed by
/// ASCII lowercase letters or digits (ADR-0002 §1). This is the rule
/// `ridl-core`'s manifest applies to package names; a service's dotted name is
/// reverse-domain in exactly the same shape (ridl §14.5).
fn is_lowercase_name_segment(segment: &str) -> bool {
    let mut chars = segment.chars();
    match chars.next() {
        Some(first) if first.is_ascii_lowercase() => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
}

fn member_name(name: Option<ast::Name>) -> Option<String> {
    Some(name?.ident_token()?.text().to_string())
}

/// The declared name of a top-level item as written, read off the tree rather
/// than off a typed node so that [`Checker::check_exposure`] stays independent
/// of the declaration kinds: a typl definition and an `interface` carry a
/// `Name`, a `service` carries its dotted `DottedName` (ridl §14.5). `None` for
/// an item that declares no name of its own — a `package` header, a bare
/// `import`, or a declaration the parser recovered without one.
fn item_declared_name(item: &ridl_syntax::SyntaxNode) -> Option<String> {
    item.children()
        .find(|child| matches!(child.kind(), SyntaxKind::Name | SyntaxKind::DottedName))
        .map(|child| significant_text(&child))
}

/// The source range of the first `PathExpr` in `clause` spelled `name`.
///
/// Presentation only: which names a clause binds to the package vocabulary is
/// decided by [`expr::collect_refs`] against the clause's own scope, and a
/// given name resolves the same way everywhere inside one clause, so the first
/// occurrence is the right place to point at.
fn clause_ref_range(clause: &ast::Expr, name: &str) -> Option<TextRange> {
    clause
        .syntax()
        .descendants()
        .filter_map(ast::PathExpr::cast)
        .find_map(|path| {
            let token = path.name_token()?;
            (token.text() == name).then(|| token.text_range())
        })
}

/// The source range of a member's name, or the whole node on a malformed
/// tree.
fn member_name_range(name: Option<ast::Name>, node: &ridl_syntax::SyntaxNode) -> TextRange {
    match name {
        Some(name) => name.syntax().text_range(),
        None => node.text_range(),
    }
}

/// The interaction kind an attribute block sits on, for the ridl §13
/// predicate table and the gf §4.3 allow-list messages.
#[derive(Clone, Copy, PartialEq, Eq)]
enum MemberKind {
    Signal,
    Event,
    Command,
    Query,
    Final,
}

impl MemberKind {
    fn noun(self) -> &'static str {
        match self {
            Self::Signal => "signal",
            Self::Event => "event",
            Self::Command => "command",
            Self::Query => "query",
            Self::Final => "final",
        }
    }
}

/// The attribute keys the general form §4.3 table defines (the key forms —
/// `require`/`ensure` parse as predicates, never as keys). A key outside
/// this list is FORM-106; inside it but not consumable here, FORM-107.
/// Recorded debt (issue #172, M3): in E2 no key is consumable, so a flat list
/// suffices; the task that first consumes a key (`persist`, `labels`, …)
/// must turn this into the full gf §4.3 key×kind allow-list.
const GF_ATTRIBUTE_KEYS: &[&str] = &[
    "default",
    "init",
    "persist",
    "invariant",
    "labels",
    "deprecated",
];

/// The Stratum-2 contract-error category names (ridl §10.2): implicit,
/// standardized, derived — never declared as error-type values. An `error`
/// enum re-declaring any of these draws RIDL-307 (reserved vocabulary).
const STRATUM_2_CATEGORIES: &[&str] = &[
    "INVALID_VALUE",
    "PRECONDITION_FAILED",
    "CONTRACT_BROKEN",
    "UNKNOWN_INTERACTION",
];

/// The `InitValue` child of an interaction node. `EventDef` and `FinalDef`
/// admit no init in the reference grammar, so the generated AST carries no
/// accessor — the lenient parse still holds the node as a direct child.
fn init_value_child(node: &ridl_syntax::SyntaxNode) -> Option<ast::InitValue> {
    node.children().find_map(ast::InitValue::cast)
}

/// The backing class of a type's backing, without emitting diagnostics (unlike
/// [`Checker::lower_backing`]). A unit backing is float-classed (§5.1); an
/// absent or malformed backing is `Unknown`.
fn backing_class(backing: Option<ast::Backing>) -> BackingClass {
    match backing {
        None => BackingClass::Unknown,
        Some(ast::Backing::Primitive(node)) => primitive_of(&node).1,
        Some(ast::Backing::Unit(_)) => BackingClass::Float,
    }
}

/// The IR primitive and backing class of a `PrimitiveType` node.
fn primitive_of(node: &ast::PrimitiveType) -> (v2::PrimitiveType, BackingClass) {
    if node.boolean_token().is_some() {
        (v2::PrimitiveType::Boolean, BackingClass::Boolean)
    } else if node.integer_token().is_some() {
        (v2::PrimitiveType::Integer, BackingClass::Integer)
    } else if node.float_token().is_some() {
        (v2::PrimitiveType::Float, BackingClass::Float)
    } else if node.string_token().is_some() {
        (v2::PrimitiveType::String, BackingClass::Str)
    } else if node.bytes_token().is_some() {
        (v2::PrimitiveType::Bytes, BackingClass::Bytes)
    } else {
        (v2::PrimitiveType::Unspecified, BackingClass::Unknown)
    }
}

fn primitive_noun(class: BackingClass) -> &'static str {
    match class {
        BackingClass::Boolean => "boolean",
        BackingClass::Integer => "integer",
        BackingClass::Float => "float",
        BackingClass::Str => "string",
        BackingClass::Bytes => "bytes",
        BackingClass::Unknown => "unknown",
    }
}

/// If the path is a single primitive keyword (`integer`, `string`, …), the
/// keyword text. Primitive keywords parse as path segments in arm position
/// (the grammar has no primitive production there).
fn primitive_path_keyword(path: &ast::PathType) -> Option<String> {
    let qualified = path.qualified_name()?;
    let tokens: Vec<_> = qualified
        .syntax()
        .children_with_tokens()
        .filter_map(|element| element.into_token())
        .filter(|token| !token.kind().is_trivia())
        .collect();
    match tokens.as_slice() {
        [token]
            if matches!(
                token.kind(),
                SyntaxKind::BooleanKw
                    | SyntaxKind::IntegerKw
                    | SyntaxKind::FloatKw
                    | SyntaxKind::StringKw
                    | SyntaxKind::BytesKw
            ) =>
        {
            Some(token.text().to_string())
        }
        _ => None,
    }
}

fn lower_reserved(entry: &ast::ReservedEntry, ordinal: u32) -> v2::Reserved {
    let name = member_name(entry.name());
    let value = entry
        .literal()
        .and_then(|literal| match literal_kind(&literal) {
            LitKind::Number { value } => exact_to_i64(&value),
            _ => None,
        });
    v2::Reserved {
        ordinal,
        name,
        value,
    }
}

fn kind_noun(kind: SymbolKind) -> &'static str {
    match kind {
        SymbolKind::Type => "type",
        SymbolKind::Const => "constant",
        SymbolKind::Struct => "struct",
        SymbolKind::Enum => "enum",
        SymbolKind::EnumSet => "enumset",
        SymbolKind::Union => "union",
        SymbolKind::Interface => "interface",
    }
}

fn kind_article(kind: SymbolKind) -> &'static str {
    match kind {
        SymbolKind::Enum | SymbolKind::EnumSet | SymbolKind::Interface => "an",
        _ => "a",
    }
}

/// The negation of the shared range-membership rule ([`scalar::range_accepts`]),
/// which the E2.11a property runner drives its corpora against. The rule lives
/// in one place so that a bug in it surfaces there rather than being
/// reimplemented identically on both sides.
fn out_of_bounds(value: &ExactValue, min: Option<&ExactValue>, max: Option<&ExactValue>) -> bool {
    !crate::scalar::range_accepts(value, min, max)
}

fn render_bound(bound: Option<&ExactValue>) -> String {
    bound
        .map(ExactValue::to_decimal_string)
        .unwrap_or_else(|| "..".to_string())
}

fn exact_is_positive(value: &ExactValue) -> bool {
    value.0.numer().sign() == num_bigint::Sign::Plus
}

fn exact_to_u64(value: &ExactValue) -> Option<u64> {
    if !value.0.is_integer() {
        return None;
    }
    u64::try_from(value.0.to_integer()).ok()
}

fn exact_to_i64(value: &ExactValue) -> Option<i64> {
    if !value.0.is_integer() {
        return None;
    }
    i64::try_from(value.0.to_integer()).ok()
}

/// Lowers a resolved [`timing::TimingSpec`] to its IR `Timing` (ADR-0008
/// decision 12): the mode discriminator, the exact-decimal microsecond bound
/// strings (an unset half-open side stays `None`), and the default-applied flag.
fn lower_timing_spec(spec: timing::TimingSpec) -> v2::Timing {
    let mode = match spec.mode {
        timing::TimingMode::StrictPeriodic => v2::TimingMode::StrictPeriodic,
        timing::TimingMode::Range => v2::TimingMode::Range,
    };
    v2::Timing {
        mode: mode as i32,
        min_us: spec.min_us.map(|value| value.to_decimal_string()),
        max_us: spec.max_us.map(|value| value.to_decimal_string()),
        default_applied: spec.default_applied,
    }
}

/// The int64 domain edge an omitted bound defaults to (§5.5).
fn int64_edge(upper: bool) -> ExactValue {
    let text = if upper {
        i64::MAX.to_string()
    } else {
        i64::MIN.to_string()
    };
    ExactValue::parse(&text).expect("the int64 edge is a valid decimal")
}

#[cfg(test)]
mod tests {
    use super::*;
    use ridl_core::db::RidlDatabase;
    use ridl_core::package::{PackageOrigin, service_catalog};
    use ridl_core::std_lib::std_package;
    use std::collections::BTreeMap;

    /// The typl reference Appendix B example, verbatim.
    const APPENDIX_B: &str = include_str!("../fixtures/appendix_b.typl");

    fn package(db: &RidlDatabase, name: &str, text: &str) -> Package {
        let file = InputFile::new(
            db,
            format!("{}.typl", name.replace('.', "/")),
            text.to_string(),
        );
        Package::new(
            db,
            name.to_string(),
            vec![file],
            PackageOrigin::WorkspaceMember,
            BTreeMap::new(),
            None,
        )
    }

    /// Checks a single-package workspace.
    fn check_source(name: &str, text: &str) -> CheckedPackage {
        let mut db = RidlDatabase::default();
        let std = std_package(&mut db);
        let pkg = package(&db, name, text);
        let ws = Workspace::new(&db, vec![pkg], BTreeMap::new());
        check_package(&db, ws, pkg, std)
    }

    /// The checker diagnostic codes, in order.
    fn codes(checked: &CheckedPackage) -> Vec<&str> {
        checked
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code.as_str())
            .collect()
    }

    /// The declaration named `name`, or a panic naming what is there.
    fn decl<'a>(checked: &'a CheckedPackage, name: &str) -> &'a v2::Decl {
        checked
            .ir
            .decls
            .iter()
            .find(|decl| decl.name == name)
            .unwrap_or_else(|| {
                panic!(
                    "no decl `{name}`; have: {:?}",
                    checked.ir.decls.iter().map(|d| &d.name).collect::<Vec<_>>()
                )
            })
    }

    fn type_def<'a>(checked: &'a CheckedPackage, name: &str) -> &'a v2::TypeDef {
        let Some(v2::decl::Kind::TypeDef(def)) = &decl(checked, name).kind else {
            panic!("`{name}` is not a type def");
        };
        def
    }

    fn struct_def<'a>(checked: &'a CheckedPackage, name: &str) -> &'a v2::StructDef {
        let Some(v2::decl::Kind::StructDef(def)) = &decl(checked, name).kind else {
            panic!("`{name}` is not a struct def");
        };
        def
    }

    fn union_def<'a>(checked: &'a CheckedPackage, name: &str) -> &'a v2::UnionDef {
        let Some(v2::decl::Kind::UnionDef(def)) = &decl(checked, name).kind else {
            panic!("`{name}` is not a union def");
        };
        def
    }

    // --- the Appendix B golden -------------------------------------------

    /// The full Appendix B package lowers end to end with no diagnostics, and
    /// its IR v2 JSON is pinned as the reviewed golden snapshot.
    #[test]
    fn appendix_b_lowers_clean_end_to_end() {
        let checked = check_source("veh.common", APPENDIX_B);
        assert!(
            checked.diagnostics.is_empty(),
            "Appendix B must lower clean, got: {:?}",
            checked.diagnostics,
        );
        assert_eq!(checked.ir.name, "veh.common");
        insta::assert_snapshot!("appendix_b_ir", v2::to_json_pretty(&checked.ir));
    }

    /// `fixed_layout` is derived per struct: every field fixed-width and
    /// non-optional, and no tombstone (typl Appendix D, FlatBuffers note).
    #[test]
    fn appendix_b_fixed_layout_is_derived() {
        let checked = check_source("veh.common", APPENDIX_B);
        assert!(struct_def(&checked, "SpeedLimitPayload").fixed_layout);
        assert!(struct_def(&checked, "SensorReading").fixed_layout);
        // DriverProfile has an optional field; SensorFault holds a
        // variable-length Message; SensorBounds holds bounded collections;
        // RawWheelFrame holds a bytes-backed `frame` field, which is not a
        // fixed C ABI even at a fixed length (C2).
        assert!(!struct_def(&checked, "DriverProfile").fixed_layout);
        assert!(!struct_def(&checked, "SensorFault").fixed_layout);
        assert!(!struct_def(&checked, "SensorBounds").fixed_layout);
        assert!(!struct_def(&checked, "RawWheelFrame").fixed_layout);
    }

    /// A struct whose field resolves to a string- or bytes-backed named type is
    /// not fixed_layout, even at a fixed length: those backings carry no fixed C
    /// ABI (C2). A struct of all fixed-width scalars stays fixed_layout.
    #[test]
    fn string_or_bytes_backed_named_field_is_not_fixed_layout() {
        let checked = check_source(
            "veh.common",
            "package veh.common\n\
             type Frame : bytes [8]\n\
             type Tag   : string [4]\n\
             type Speed : km/h [0.0..250.0 step 0.5]\n\
             struct HasBytes  { frame : Frame }\n\
             struct HasString { tag : Tag }\n\
             struct AllScalar { a : Speed b : Speed }\n",
        );
        assert!(!struct_def(&checked, "HasBytes").fixed_layout);
        assert!(!struct_def(&checked, "HasString").fixed_layout);
        assert!(struct_def(&checked, "AllScalar").fixed_layout);
    }

    // --- first-wins lowering (cross-seam fact 2) --------------------------

    /// A duplicate declaration lowers once: the IR carries exactly the first
    /// declaration (the resolver's winner, ADR-0007 decision 6).
    #[test]
    fn duplicate_declaration_lowers_only_the_first() {
        let checked = check_source(
            "app",
            "package app\ntype Speed: km/h [0.0..250.0 step 0.5]\ntype Speed: m/s [0.0..70.0 step 0.5]\n",
        );
        let speeds: Vec<_> = checked
            .ir
            .decls
            .iter()
            .filter(|decl| decl.name == "Speed")
            .collect();
        assert_eq!(speeds.len(), 1, "the duplicate lowers exactly once");
        let Some(v2::decl::Kind::TypeDef(def)) = &speeds[0].kind else {
            panic!("Speed is a type def");
        };
        assert_eq!(
            def.backing.as_ref().unwrap().kind,
            Some(v2::backing::Kind::Unit("km/h".to_string())),
            "the first declaration's unit wins",
        );
        // The TYPL-009 itself is the resolver's diagnostic, not the checker's.
        assert!(
            !codes(&checked).contains(&"TYPL-009"),
            "the checker does not repeat the resolver's TYPL-009",
        );
    }

    // --- backing lowering -------------------------------------------------

    /// A primitive-backed type lowers to `Backing { primitive }` — the E0
    /// `unit: "integer"` artifact must not carry into IR v2.
    #[test]
    fn primitive_backing_lowers_to_backing_primitive() {
        let checked = check_source("app", "package app\ntype Counter : integer [0..65535]\n");
        let def = type_def(&checked, "Counter");
        assert_eq!(
            def.backing.as_ref().unwrap().kind,
            Some(v2::backing::Kind::Primitive(
                v2::PrimitiveType::Integer as i32
            )),
        );
        assert_eq!(
            def.width,
            Some(v2::type_def::Width::IntWidth(v2::IntWidth::U16 as i32)),
        );
        let constraint = def.constraint.as_ref().unwrap();
        assert_eq!(constraint.min.as_deref(), Some("0"));
        assert_eq!(constraint.max.as_deref(), Some("65535"));
    }

    /// A unit-backed type lowers to `Backing { unit }` with the canonical
    /// UCUM expression.
    #[test]
    fn unit_backing_lowers_to_canonical_unit() {
        let checked = check_source(
            "app",
            "package app\ntype Speed : km/h [0.0..250.0 step 0.5]\n",
        );
        let def = type_def(&checked, "Speed");
        assert_eq!(
            def.backing.as_ref().unwrap().kind,
            Some(v2::backing::Kind::Unit("km/h".to_string())),
        );
        assert_eq!(
            def.width,
            Some(v2::type_def::Width::FloatWidth(v2::FloatWidth::F32 as i32)),
            "[0.0..250.0 step 0.5] has 501 values and representable bounds",
        );
        let constraint = def.constraint.as_ref().unwrap();
        assert_eq!(constraint.step.as_deref(), Some("0.5"));
    }

    // --- TYPL-101/102/103: missing-constraint warnings (§4) ---------------

    #[test]
    fn typl_101_integer_without_range_warns() {
        let checked = check_source("app", "package app\ntype Counter : integer\n");
        assert_eq!(codes(&checked), vec!["TYPL-101"]);
        assert_eq!(checked.diagnostics[0].severity, Severity::Warning);
        // "no range" derives int64 (§4.2 last row).
        assert_eq!(
            type_def(&checked, "Counter").width,
            Some(v2::type_def::Width::IntWidth(v2::IntWidth::I64 as i32)),
        );
    }

    #[test]
    fn typl_102_float_without_step_warns() {
        let checked = check_source("app", "package app\ntype Gain : float [0.0..1.0]\n");
        assert_eq!(codes(&checked), vec!["TYPL-102"]);
        assert_eq!(checked.diagnostics[0].severity, Severity::Warning);
        // No step: count-based inference falls back to float64 (§4.3).
        assert_eq!(
            type_def(&checked, "Gain").width,
            Some(v2::type_def::Width::FloatWidth(v2::FloatWidth::F64 as i32)),
        );
    }

    /// §4.4: a string without explicit bounds warns and the `[0..256]`
    /// default is applied in the IR.
    #[test]
    fn typl_103_string_without_bounds_gets_the_default() {
        let checked = check_source("app", "package app\ntype Tag : string\n");
        assert_eq!(codes(&checked), vec!["TYPL-103"]);
        assert_eq!(checked.diagnostics[0].severity, Severity::Warning);
        let constraint = type_def(&checked, "Tag").constraint.as_ref().unwrap();
        assert_eq!(constraint.len_min, Some(0));
        assert_eq!(constraint.len_max, Some(256));
    }

    // --- TYPL-104/105: range and step validation --------------------------

    #[test]
    fn typl_104_min_greater_than_max() {
        let checked = check_source("app", "package app\ntype Bad : integer [10..5]\n");
        assert_eq!(codes(&checked), vec!["TYPL-104"]);
        assert_eq!(checked.diagnostics[0].severity, Severity::Error);
    }

    #[test]
    fn typl_105_step_on_an_integer_type() {
        let checked = check_source("app", "package app\ntype Bad : integer [0..10 step 2]\n");
        assert_eq!(codes(&checked), vec!["TYPL-105"]);
    }

    /// The T10 review fact: `1` and `1.0` are one `ExactValue`, so the
    /// integer-form step on a float type is detected from the raw CST token,
    /// never from the decimal rendering.
    #[test]
    fn typl_105_integer_form_step_on_a_float_type() {
        let checked = check_source("app", "package app\ntype Bad : float [0.0..10.0 step 1]\n");
        assert_eq!(codes(&checked), vec!["TYPL-105"]);
        // The float-form spelling of the same value is fine.
        let ok = check_source("app", "package app\ntype Ok : float [0.0..10.0 step 1.0]\n");
        assert!(codes(&ok).is_empty(), "got: {:?}", ok.diagnostics);
    }

    #[test]
    fn typl_105_non_positive_and_oversized_steps() {
        let zero = check_source(
            "app",
            "package app\ntype Bad : float [0.0..10.0 step 0.0]\n",
        );
        assert_eq!(codes(&zero), vec!["TYPL-105"]);
        let large = check_source(
            "app",
            "package app\ntype Bad : float [0.0..10.0 step 20.0]\n",
        );
        assert_eq!(codes(&large), vec!["TYPL-105"]);
    }

    // --- TYPL-108/109: const values and declared inits --------------------

    #[test]
    fn typl_108_const_value_outside_its_type_range() {
        let checked = check_source(
            "app",
            "package app\ntype Speed: km/h [0.0..250.0 step 0.5]\nconst TOO_FAST: Speed = 300.0\n",
        );
        assert_eq!(codes(&checked), vec!["TYPL-108"]);
        assert!(checked.diagnostics[0].message.contains("TOO_FAST"));
        assert!(checked.diagnostics[0].message.contains("Speed"));
        // The value is representable, so the const still lowers.
        let Some(v2::decl::Kind::ConstDef(def)) = &decl(&checked, "TOO_FAST").kind else {
            panic!("TOO_FAST is a const def");
        };
        assert_eq!(def.value, "300");
        assert_eq!(def.type_ref.as_deref(), Some("Speed"));
    }

    #[test]
    fn typl_109_declared_init_outside_the_range() {
        let checked = check_source(
            "app",
            "package app\ntype Speed: km/h [0.0..250.0 step 0.5] = 300.0\n",
        );
        assert_eq!(codes(&checked), vec!["TYPL-109"]);
        // The declared init still lowers; the diagnostic marks the failure.
        let def = type_def(&checked, "Speed");
        assert_eq!(def.declared_init.as_deref(), Some("300"));
        assert_eq!(
            def.init,
            Some(v2::InitValue {
                derivable: true,
                value: Some("300".to_string()),
            }),
        );
    }

    /// A valid declared init lowers into `InitValue`; a declaration without one
    /// derives its init from the §5.8 table (E1.9) while keeping
    /// `declared_init` absent.
    #[test]
    fn declared_init_lowers_and_underived_init_is_derived() {
        let checked = check_source(
            "app",
            "package app\ntype Speed: km/h [0.0..250.0 step 0.5] = 0.0\ntype Gain: float [0.0..1.0 step 0.01]\n",
        );
        assert!(codes(&checked).is_empty(), "got: {:?}", checked.diagnostics);
        assert_eq!(
            type_def(&checked, "Speed").init,
            Some(v2::InitValue {
                derivable: true,
                value: Some("0".to_string()),
            }),
        );
        // Gain declares no init, so it derives `0` (within `[0.0..1.0]`) —
        // rendered as the canonical `0` — with `declared_init` still absent.
        assert_eq!(
            type_def(&checked, "Gain").init,
            Some(v2::InitValue {
                derivable: true,
                value: Some("0".to_string()),
            }),
        );
        assert_eq!(type_def(&checked, "Gain").declared_init, None);
    }

    /// A bound written as an undefined constant defers the width — a definite
    /// width would flip once the constant resolves — and now also reports the
    /// unknown constant (the T13 bound-diagnostic closure). §16.2 defines no
    /// bound-specific code, so the message is the T6 codeless unknown-reference
    /// shape.
    #[test]
    fn written_but_unresolved_bound_defers_and_reports() {
        let checked = check_source("app", "package app\ntype X : integer [0..TYPO]\n");
        let def = type_def(&checked, "X");
        assert_eq!(
            def.width, None,
            "no definite width from an unresolved bound"
        );
        let constraint = def.constraint.as_ref().unwrap();
        assert_eq!(constraint.min.as_deref(), Some("0"));
        assert_eq!(
            constraint.max, None,
            "the unresolved bound lowers as absent"
        );
        assert_eq!(checked.diagnostics.len(), 1);
        assert!(
            checked.diagnostics[0].code.is_empty(),
            "no §16 code for a bound const"
        );
        assert_eq!(
            checked.diagnostics[0].message,
            "unknown constant `TYPO` in range bound",
        );

        // The float path has the same shape.
        let float = check_source("app", "package app\ntype Y : float [0.0..TYPO step 0.5]\n");
        assert_eq!(type_def(&float, "Y").width, None);
        assert_eq!(codes(&float), vec![""], "got: {:?}", float.diagnostics);
        assert!(float.diagnostics[0].message.contains("TYPO"));
    }

    /// A chained constant bound (`const MAX = BASE`) resolves through the const
    /// chain: the width derives and no diagnostic fires. The cycle guard keeps a
    /// self-referential constant used in a bound from looping — it reports the
    /// unknown-constant message once instead.
    #[test]
    fn chained_constant_bound_resolves_and_cycle_is_guarded() {
        let chained = check_source(
            "app",
            "package app\nconst BASE = 255\nconst MAX = BASE\ntype X : integer [0..MAX]\n",
        );
        assert!(
            codes(&chained).is_empty(),
            "the chained const resolves, got: {:?}",
            chained.diagnostics,
        );
        assert_eq!(
            type_def(&chained, "X").width,
            Some(v2::type_def::Width::IntWidth(v2::IntWidth::U8 as i32)),
            "[0..255] via the const chain derives uint8",
        );

        // A self-referential constant must not loop; the bound reports it once.
        let cyclic = check_source(
            "app",
            "package app\nconst LOOP = LOOP\ntype Y : integer [0..LOOP]\n",
        );
        assert_eq!(codes(&cyclic), vec![""], "got: {:?}", cyclic.diagnostics);
        assert!(cyclic.diagnostics[0].message.contains("LOOP"));
        assert_eq!(type_def(&cyclic, "Y").width, None);
    }

    /// A range bound that references a non-numeric constant borrows TYPL-105 —
    /// the documented borrow, since §16.2 scopes 105 to `step` and defines no
    /// code for a malformed bound constant.
    #[test]
    fn non_numeric_constant_bound_borrows_typl_105() {
        let checked = check_source(
            "app",
            "package app\nconst FLAG = true\ntype X : integer [0..FLAG]\n",
        );
        assert_eq!(codes(&checked), vec!["TYPL-105"]);
        assert!(checked.diagnostics[0].message.contains("FLAG"));
        assert_eq!(type_def(&checked, "X").width, None);
    }

    /// §5.5: a bound genuinely omitted from source still takes the
    /// widest-value default and derives a width.
    #[test]
    fn omitted_bound_still_defaults_to_the_int64_edge() {
        let checked = check_source("app", "package app\ntype X : integer [0..]\n");
        assert!(codes(&checked).is_empty(), "got: {:?}", checked.diagnostics);
        assert_eq!(
            type_def(&checked, "X").width,
            Some(v2::type_def::Width::IntWidth(v2::IntWidth::U64 as i32)),
            "[0..] fills the open side with the int64 edge and derives uint64",
        );
    }

    // --- TYPL-110/111: units and the int64 domain -------------------------

    #[test]
    fn typl_110_unknown_unit() {
        let checked = check_source("app", "package app\ntype X : xyz [0.0..1.0 step 0.1]\n");
        assert_eq!(codes(&checked), vec!["TYPL-110"]);
    }

    #[test]
    fn typl_111_integer_range_outside_int64() {
        let checked = check_source(
            "app",
            "package app\ntype X : integer [0..9223372036854775808]\n",
        );
        assert_eq!(codes(&checked), vec!["TYPL-111"]);
        assert_eq!(
            type_def(&checked, "X").width,
            None,
            "no width fits a range outside the int64 domain",
        );
    }

    // --- unknown type references (T6 description-first shape, no code) ----

    #[test]
    fn unknown_type_reference_keeps_the_description_first_shape() {
        let checked = check_source("app", "package app\nconst X: Missing = 1.0\n");
        assert_eq!(checked.diagnostics.len(), 1);
        assert!(checked.diagnostics[0].code.is_empty(), "no §16 code exists");
        assert_eq!(
            checked.diagnostics[0].message,
            "unknown type name `Missing`",
        );
    }

    #[test]
    fn type_reference_naming_a_const_is_description_first() {
        let checked = check_source(
            "app",
            "package app\ntype Speed: km/h [0.0..250.0 step 0.5]\nconst MAX_SPEED: Speed = 250.0\nconst A: MAX_SPEED = 1.0\n",
        );
        assert_eq!(checked.diagnostics.len(), 1);
        assert!(checked.diagnostics[0].code.is_empty());
        assert_eq!(
            checked.diagnostics[0].message,
            "expected a type, but `MAX_SPEED` names a constant",
        );
    }

    // --- TYPL-201/202: collection bounds ----------------------------------

    #[test]
    fn typl_201_array_without_bounds() {
        let checked = check_source(
            "app",
            "package app\ntype Speed: km/h [0.0..250.0 step 0.5]\nstruct S { xs : [Speed] }\n",
        );
        assert!(
            codes(&checked).contains(&"TYPL-201"),
            "got: {:?}",
            checked.diagnostics,
        );
    }

    #[test]
    fn typl_202_map_without_bounds() {
        let checked = check_source("app", "package app\nstruct S { m : [Label : Name] }\n");
        assert!(
            codes(&checked).contains(&"TYPL-202"),
            "got: {:?}",
            checked.diagnostics,
        );
    }

    // --- TYPL-203: enum values --------------------------------------------

    #[test]
    fn typl_203_duplicate_enum_value() {
        let checked = check_source("app", "package app\nenum E { A = 0, B = 0 }\n");
        assert_eq!(codes(&checked), vec!["TYPL-203"]);
    }

    // --- TYPL-204: union arm shape ----------------------------------------

    #[test]
    fn typl_204_union_arm_with_primitive() {
        let checked = check_source(
            "app",
            "package app\nerror struct Fault { code : Counter }\ntype Counter : integer [0..255]\nunion U { ok : integer, err : Fault }\n",
        );
        assert_eq!(codes(&checked), vec!["TYPL-204"]);
    }

    // --- TYPL-206: recursive composites -----------------------------------

    #[test]
    fn typl_206_direct_recursion() {
        let checked = check_source("app", "package app\nstruct S { next : S }\n");
        assert_eq!(codes(&checked), vec!["TYPL-206"]);
    }

    #[test]
    fn typl_206_transitive_recursion() {
        let checked = check_source(
            "app",
            "package app\nstruct S { t : T }\nstruct T { s : [S; 4] }\n",
        );
        // Both composites sit on the cycle, so each reports once — exactly
        // two TYPL-206, nothing else.
        assert_eq!(
            codes(&checked),
            vec!["TYPL-206", "TYPL-206"],
            "got: {:?}",
            checked.diagnostics,
        );
    }

    /// The reviewer's qualified-ref-only cross-package cycle: no imports, so
    /// the resolver's TYPL-004 (import-edge walk) cannot see it — the
    /// composite-reference DFS must.
    #[test]
    fn typl_206_cross_package_qualified_cycle() {
        let mut db = RidlDatabase::default();
        let std = std_package(&mut db);
        let a = package(&db, "a", "package a\nstruct S { x : b.T }\n");
        let b = package(&db, "b", "package b\nstruct T { y : a.S }\n");
        let ws = Workspace::new(&db, vec![a, b], BTreeMap::new());

        let checked = check_package(&db, ws, a, std);
        assert_eq!(codes(&checked), vec!["TYPL-206"]);
    }

    // --- TYPL-207 + the bit-position domain -------------------------------

    #[test]
    fn typl_207_duplicate_enumset_bits() {
        let checked = check_source("app", "package app\nenumset W { A = 0, B = 0 }\n");
        assert_eq!(codes(&checked), vec!["TYPL-207"]);
    }

    /// `enumset_width` saturates past bit 63 with no error (the T10 review
    /// fact); the checker rejects the position itself — int64 domain.
    #[test]
    fn enumset_bit_past_63_is_typl_111() {
        let checked = check_source("app", "package app\nenumset W { A = 64 }\n");
        assert_eq!(codes(&checked), vec!["TYPL-111"]);
    }

    #[test]
    fn derived_enumset_copies_bits_and_width() {
        let checked = check_source(
            "app",
            "package app\nenum Warning { LOW_FUEL = 0, CHECK_ENGINE = 1 }\nenumset WarningFlags : Warning\n",
        );
        assert!(codes(&checked).is_empty(), "got: {:?}", checked.diagnostics);
        let Some(v2::decl::Kind::EnumSetDef(def)) = &decl(&checked, "WarningFlags").kind else {
            panic!("WarningFlags is an enumset def");
        };
        assert_eq!(def.backing_enum.as_deref(), Some("Warning"));
        assert_eq!(
            def.bits.iter().map(|b| b.value).collect::<Vec<_>>(),
            vec![0, 1],
            "the derived form copies the backing enum's values",
        );
        assert_eq!(def.width, v2::IntWidth::U8 as i32);
    }

    // --- TYPL-208/209: string/bytes fields and map keys -------------------

    #[test]
    fn typl_208_bare_string_field() {
        let checked = check_source("app", "package app\nstruct S { s : string }\n");
        assert_eq!(codes(&checked), vec!["TYPL-208"]);
    }

    /// `bytes [8]` is an inline constrained scalar (Appendix B's
    /// `RawWheelFrame`), not a direct primitive use — no TYPL-208.
    #[test]
    fn constrained_bytes_field_is_an_inline_scalar() {
        let checked = check_source("app", "package app\nstruct S { frame : bytes [8] }\n");
        assert!(codes(&checked).is_empty(), "got: {:?}", checked.diagnostics);
        let v2::struct_member::Member::Field(field) = struct_def(&checked, "S").members[0]
            .member
            .as_ref()
            .unwrap()
        else {
            panic!("expected a field");
        };
        let Some(v2::field_type::Kind::InlineScalar(inline)) = &field.r#type.as_ref().unwrap().kind
        else {
            panic!("expected an inline scalar");
        };
        let constraint = inline.constraint.as_ref().unwrap();
        assert_eq!(constraint.len_min, Some(8));
        assert_eq!(constraint.len_max, Some(8));
    }

    #[test]
    fn typl_209_map_key_not_a_named_string_type() {
        let checked = check_source(
            "app",
            "package app\ntype Speed: km/h [0.0..250.0 step 0.5]\nstruct S { m : [Speed : Name; 0..4] }\n",
        );
        assert_eq!(codes(&checked), vec!["TYPL-209"]);
    }

    // --- TYPL-210/211: reserved tombstones (§7.4) -------------------------

    /// The §7.4 tombstone example, verbatim: ordinals count reserved slots,
    /// 1-based.
    #[test]
    fn tombstone_example_assigns_ordinals_around_the_slot() {
        let checked = check_source(
            "app",
            "package app\n\
             type Speed: km/h [0.0..250.0 step 0.5]\n\
             struct DriverProfile {\n\
               name     : Name\n\
               reserved legacyChecksum      // was ordinal 2 — slot retired, never reused\n\
               speed    : Speed\n\
             }\n",
        );
        assert!(codes(&checked).is_empty(), "got: {:?}", checked.diagnostics);
        let members = &struct_def(&checked, "DriverProfile").members;
        assert_eq!(members.len(), 3);
        let v2::struct_member::Member::Field(name_field) = members[0].member.as_ref().unwrap()
        else {
            panic!("member 1 is a field");
        };
        assert_eq!((name_field.name.as_str(), name_field.ordinal), ("name", 1));
        let v2::struct_member::Member::Reserved(tombstone) = members[1].member.as_ref().unwrap()
        else {
            panic!("member 2 is reserved");
        };
        assert_eq!(tombstone.ordinal, 2);
        assert_eq!(tombstone.name.as_deref(), Some("legacyChecksum"));
        let v2::struct_member::Member::Field(speed_field) = members[2].member.as_ref().unwrap()
        else {
            panic!("member 3 is a field");
        };
        assert_eq!(
            (speed_field.name.as_str(), speed_field.ordinal),
            ("speed", 3)
        );
    }

    #[test]
    fn typl_210_field_redeclared_under_reserved() {
        let checked = check_source(
            "app",
            "package app\nstruct S { reserved legacyChecksum\n legacyChecksum : Name }\n",
        );
        assert_eq!(codes(&checked), vec!["TYPL-210"]);
    }

    /// `reserved` in enum bodies retires the integer value (§7.4): reusing it
    /// is TYPL-210.
    #[test]
    fn typl_210_enum_value_redeclared_under_reserved() {
        let checked = check_source("app", "package app\nenum E { A = 0, reserved 3, B = 3 }\n");
        assert_eq!(codes(&checked), vec!["TYPL-210"]);
    }

    #[test]
    fn typl_211_duplicate_reserved_entry() {
        let checked = check_source(
            "app",
            "package app\nstruct S { reserved legacy\n reserved legacy\n x : Name }\n",
        );
        assert_eq!(codes(&checked), vec!["TYPL-211"]);
        assert_eq!(checked.diagnostics[0].severity, Severity::Warning);
    }

    // --- TYPL-212/213/214: error vocabulary and result unions -------------

    #[test]
    fn typl_212_error_on_a_type_definition() {
        let checked = check_source("app", "package app\nerror type X : integer [0..1]\n");
        assert_eq!(codes(&checked), vec!["TYPL-212"]);
    }

    #[test]
    fn result_union_two_arms_sets_is_result() {
        let checked = check_source(
            "app",
            "package app\n\
             type Speed: km/h [0.0..250.0 step 0.5]\n\
             struct Reading { value : Speed }\n\
             error struct Fault { message : Message }\n\
             union Outcome { ok : Reading, err : Fault }\n",
        );
        assert!(codes(&checked).is_empty(), "got: {:?}", checked.diagnostics);
        let def = union_def(&checked, "Outcome");
        assert!(
            def.is_result,
            "one success + one error arm is a result union"
        );
        assert_eq!(
            def.arms.iter().map(|arm| arm.ordinal).collect::<Vec<_>>(),
            vec![1, 2],
        );
    }

    /// A three-arm mix of error and non-error arms is not the result-union
    /// shape (§10.2).
    #[test]
    fn typl_213_three_arm_mix_is_rejected() {
        let checked = check_source(
            "app",
            "package app\n\
             type Speed: km/h [0.0..250.0 step 0.5]\n\
             struct Reading { value : Speed }\n\
             struct Extra { value : Speed }\n\
             error struct Fault { message : Message }\n\
             union Outcome { ok : Reading, extra : Extra, err : Fault }\n",
        );
        assert_eq!(codes(&checked), vec!["TYPL-213"]);
        assert!(!union_def(&checked, "Outcome").is_result);
    }

    #[test]
    fn typl_214_error_union_with_non_error_arm() {
        let checked = check_source(
            "app",
            "package app\n\
             error enum DiagError { FILTER_INVALID = 0 }\n\
             struct Plain { name : Name }\n\
             error union ServiceFault { diag : DiagError, plain : Plain }\n",
        );
        assert_eq!(codes(&checked), vec!["TYPL-214"]);
    }

    // --- const_value: the shipped interface (task 15 consumes it) ---------

    /// `const_value` follows `const = const` chains to a literal and guards
    /// cycles with a `(package, name)` visited set.
    #[test]
    fn const_value_follows_chains_and_guards_cycles() {
        let mut db = RidlDatabase::default();
        let std = std_package(&mut db);
        let app = package(
            &db,
            "app",
            "package app\nconst BASE = 42\nconst MID = BASE\nconst TOP = MID\nconst LOOP = LOOP\n",
        );
        let ws = Workspace::new(&db, vec![app], BTreeMap::new());
        let resolution = resolve_package(&db, ws, app, std);

        assert_eq!(
            const_value(&db, ws, std, &resolution, "TOP"),
            Some(ConstValue::Number(ExactValue::parse("42").unwrap())),
            "a const->const->const chain resolves to the literal",
        );
        assert_eq!(
            const_value(&db, ws, std, &resolution, "LOOP"),
            None,
            "a self-referential const is cycle-guarded to None",
        );
        assert_eq!(const_value(&db, ws, std, &resolution, "MISSING"), None);
    }

    // --- TYPL-108: §5.7 nominal identity for const inits ------------------

    /// §5.7: a const of a named type is never initialized from a value of
    /// another named type. `Speed` and `Torque` are both float-backed and
    /// nominally distinct — a `Speed` const initialized from a `Torque` const
    /// is TYPL-108.
    #[test]
    fn typl_108_nominal_const_init_from_a_different_named_type() {
        let checked = check_source(
            "app",
            "package app\n\
             type Speed  : km/h [0.0..250.0 step 0.5]\n\
             type Torque : N.m  [0.0..1000.0 step 0.5]\n\
             const MAX_TORQUE : Torque = 1000.0\n\
             const FAST       : Speed  = MAX_TORQUE\n",
        );
        assert_eq!(codes(&checked), vec!["TYPL-108"]);
        assert!(checked.diagnostics[0].message.contains("Speed"));
        assert!(checked.diagnostics[0].message.contains("Torque"));
    }

    /// A const of a named type initialized from another const of the *same*
    /// named type is fine (nominal identity holds).
    #[test]
    fn same_named_type_const_init_is_clean() {
        let checked = check_source(
            "app",
            "package app\n\
             type Speed : km/h [0.0..250.0 step 0.5]\n\
             const MAX_SPEED : Speed = 250.0\n\
             const CRUISE    : Speed = MAX_SPEED\n",
        );
        assert!(codes(&checked).is_empty(), "got: {:?}", checked.diagnostics);
        // The clean verdict was the whole of this assertion until issue #170,
        // and it is the half that hid the defect: `CRUISE` lowered as the text
        // `"MAX_SPEED"`, which the Rust backend emitted as `Speed(MAX_SPEED)`
        // and rustc rejected. A `const = const` that checks clean must also
        // *lower* to something a backend can emit.
        assert_eq!(const_def(&checked, "CRUISE").value, "250");
    }

    /// The `ConstDef` named `name`, or a panic naming what is there.
    fn const_def<'a>(checked: &'a CheckedPackage, name: &str) -> &'a v2::ConstDef {
        let Some(v2::decl::Kind::ConstDef(def)) = &decl(checked, name).kind else {
            panic!("`{name}` is not a const def");
        };
        def
    }

    /// **Issue #170.** A constant whose value is a constant reference lowers as
    /// the referenced **value**, in every kind a constant can hold.
    ///
    /// `ConstDef.value` is a bare string with no discriminator, so a reference
    /// left unresolved in it is indistinguishable from a text value spelling
    /// the same name: `const A : string = "B"` and `const A : string = B`
    /// lowered to the identical IR. That is why the repair belongs here and not
    /// in a backend — and why the backends each got it wrong differently, one
    /// emitting uncompilable Rust, one emitting a silently wrong value, one
    /// refusing the package with a message about integer width.
    ///
    /// **Every kind is asserted, not the reported one.** The report named a
    /// named-scalar constant. Bool and text were *silently wrong* rather than
    /// uncompilable, which is worse and would have survived a fix verified by
    /// compiling.
    #[test]
    fn a_constant_reference_lowers_as_the_referenced_value() {
        let checked = check_source(
            "app",
            "package app\n\
             type Tick : integer [0..100]\n\
             const SECRET   : Tick    = 7\n\
             const NAMED    : Tick    = SECRET\n\
             const WHOLE    : integer = 7\n\
             const INT      : integer = WHOLE\n\
             const RATIO    : float   = 3.5\n\
             const FLOAT    : float   = RATIO\n\
             const FLAG     : boolean = true\n\
             const BOOL     : boolean = FLAG\n\
             const GREETING : string  = \"hello\"\n\
             const TEXT     : string  = GREETING\n\
             const VIN_PATTERN = /^[A-Z0-9]{17}$/\n\
             const ALIAS       = VIN_PATTERN\n\
             const HOP      : integer = INT\n",
        );
        assert!(codes(&checked).is_empty(), "got: {:?}", checked.diagnostics);

        for (name, expected) in [
            ("NAMED", "7"),
            ("INT", "7"),
            ("FLOAT", "3.5"),
            ("BOOL", "true"),
            ("TEXT", "hello"),
            // Two hops: the chain is followed to the literal, not one step.
            ("HOP", "7"),
        ] {
            assert_eq!(
                const_def(&checked, name).value,
                expected,
                "`{name}` must lower as the referenced value, not its name",
            );
            assert_eq!(const_def(&checked, name).regex, None);
        }

        // A reference to a regex constant becomes a regex constant, with the
        // `/…/` delimiters the direct form carries.
        assert_eq!(
            const_def(&checked, "ALIAS").regex.as_deref(),
            Some("/^[A-Z0-9]{17}$/"),
        );
        assert_eq!(const_def(&checked, "ALIAS").value, "");
    }

    /// A cross-package constant reference resolves in the package that declares
    /// the constant holding it (typl §3.2), so an imported constant lowers as a
    /// value too. A backend sees one package's IR at a time and could not have
    /// resolved this at all, whichever way it tried.
    #[test]
    fn a_cross_package_constant_reference_lowers_as_the_referenced_value() {
        let mut db = RidlDatabase::default();
        let std = std_package(&mut db);
        let common = package(
            &db,
            "veh.common",
            "package veh.common\ntype Speed : km/h [0.0..250.0 step 0.5]\nconst MAX_SPEED : Speed = 250.0\n",
        );
        let cluster = package(
            &db,
            "veh.cluster",
            "package veh.cluster\nimport veh.common.Speed\nimport veh.common.MAX_SPEED\nconst CRUISE : Speed = MAX_SPEED\n",
        );
        let ws = Workspace::new(&db, vec![common, cluster], BTreeMap::new());

        let checked = check_package(&db, ws, cluster, std);
        assert!(codes(&checked).is_empty(), "got: {:?}", checked.diagnostics);
        assert_eq!(const_def(&checked, "CRUISE").value, "250");
    }

    /// A constant reference that does not resolve keeps the written text, which
    /// is what it did before issue #170 and must keep doing: the resolver owns
    /// the diagnostic for an unknown name, and lowering must not invent a value
    /// for a name it cannot follow. A self-referential constant is the same
    /// case — the cycle guard in [`const_value`] returns `None`.
    #[test]
    fn an_unresolvable_constant_reference_keeps_the_written_text() {
        let cyclic = check_source("app", "package app\nconst LOOP : integer = LOOP\n");
        assert_eq!(
            const_def(&cyclic, "LOOP").value,
            "LOOP",
            "a cycle-guarded reference lowers as the written text, not as a guessed value",
        );

        let unknown = check_source("app", "package app\nconst GHOST : integer = MISSING\n");
        assert_eq!(const_def(&unknown, "GHOST").value, "MISSING");
    }

    /// **Issue #170, the declared-init half.** A constant reference in a
    /// declared init (`= value`, typl §5.8, §6) lowers as the referenced value
    /// for every kind, not only the numeric one.
    ///
    /// The numeric case already resolved, which is exactly what made this hard
    /// to see: `type T : integer [0..100] = SEVEN` was right and
    /// `type T : string [1..20] = GREETING` lowered the *name* as the string
    /// value, so the generated `Default` returned `"GREETING"`. One rule, two
    /// implementations, agreeing on the case anyone would test.
    #[test]
    fn a_constant_reference_in_a_declared_init_lowers_as_the_referenced_value() {
        let checked = check_source(
            "app",
            "package app\n\
             const SEVEN    : integer = 7\n\
             const GREETING : string  = \"hello\"\n\
             const FLAG     : boolean = true\n\
             type Counted : integer [0..100]  = SEVEN\n\
             type Labelled : string [1..20]   = GREETING\n\
             type Enabled : boolean           = FLAG\n\
             struct Holder {\n\
             \x20 count : integer [0..100] = SEVEN\n\
             \x20 label : string [1..20]   = GREETING\n\
             \x20 enabled : boolean        = FLAG\n\
             }\n",
        );
        assert!(codes(&checked).is_empty(), "got: {:?}", checked.diagnostics);

        for (name, expected) in [("Counted", "7"), ("Labelled", "hello"), ("Enabled", "true")] {
            let td = type_def(&checked, name);
            assert_eq!(
                td.declared_init.as_deref(),
                Some(expected),
                "`type {name}`'s declared init must lower as the referenced value",
            );
            assert_eq!(
                td.init.as_ref().and_then(|init| init.value.as_deref()),
                Some(expected),
            );
        }

        let holder = struct_def(&checked, "Holder");
        let fields: Vec<(&str, Option<&str>)> = holder
            .members
            .iter()
            .filter_map(|member| match &member.member {
                Some(v2::struct_member::Member::Field(field)) => {
                    Some((field.name.as_str(), field.declared_init.as_deref()))
                }
                _ => None,
            })
            .collect();
        assert_eq!(
            fields,
            [
                ("count", Some("7")),
                ("label", Some("hello")),
                ("enabled", Some("true")),
            ],
            "a struct field's declared init resolves the same way a type's does",
        );
    }

    // --- TYPL-106: regex validation (regress) -----------------------------

    #[test]
    fn typl_106_invalid_regex_constant() {
        let checked = check_source("app", "package app\nconst BAD = /[/\n");
        assert_eq!(codes(&checked), vec!["TYPL-106"]);
    }

    #[test]
    fn typl_106_invalid_inline_match_regex() {
        let checked = check_source("app", "package app\ntype Bad : string [1..10 match /[/]\n");
        // The invalid inline regex is TYPL-106; the `match`-typed `Bad` also has
        // no derivable init and no declared `= value`, so it is TYPL-115 (info).
        assert_eq!(codes(&checked), vec!["TYPL-106", "TYPL-115"]);
    }

    /// A valid regex constant reused in a `match` bound raises no TYPL-106 — the
    /// constant is validated at its declaration, not re-validated at the use
    /// site. A `match`-typed type is not init-derivable, so `Vin` is TYPL-115
    /// (info) rather than clean.
    #[test]
    fn valid_regex_constant_and_match_are_info_only() {
        let checked = check_source(
            "app",
            "package app\nconst VIN = /^[A-HJ-NPR-Z0-9]{17}$/\ntype Vin : string [17 match VIN]\n",
        );
        assert_eq!(codes(&checked), vec!["TYPL-115"]);
        assert_eq!(checked.diagnostics[0].severity, Severity::Info);
        assert_eq!(
            type_def(&checked, "Vin").init,
            Some(v2::InitValue {
                derivable: false,
                value: None,
            }),
        );
    }

    // --- TYPL-005: internal-type exposure ---------------------------------

    #[test]
    fn typl_005_public_struct_exposes_internal_field_type() {
        let checked = check_source(
            "app",
            "package app\ninternal type Secret : integer [0..10]\nstruct Public { s : Secret }\n",
        );
        assert_eq!(codes(&checked), vec!["TYPL-005"]);
        assert!(checked.diagnostics[0].message.contains("Secret"));
    }

    /// An `internal` declaration may reference an `internal` type freely — the
    /// contract surface is not widened.
    #[test]
    fn internal_declaration_may_reference_an_internal_type() {
        let checked = check_source(
            "app",
            "package app\ninternal type Secret : integer [0..10]\ninternal struct Helper { s : Secret }\n",
        );
        assert!(codes(&checked).is_empty(), "got: {:?}", checked.diagnostics);
    }

    /// A public type exposing an `internal` constant in a range bound is
    /// TYPL-005 (§3.3 "bounds constants").
    #[test]
    fn typl_005_public_type_exposes_internal_bound_constant() {
        let checked = check_source(
            "app",
            "package app\ninternal const SECRET_MAX = 100\ntype Level : integer [0..SECRET_MAX]\n",
        );
        assert_eq!(codes(&checked), vec!["TYPL-005"]);
        assert!(checked.diagnostics[0].message.contains("SECRET_MAX"));
    }

    // --- TYPL-005 and RIDL-143 on the interaction layer (issue #161) -------
    //
    // The exposure rule used to be a step of the typl definition lowering, so
    // the two declaration kinds E2 added — `interface` and `service` — were
    // never checked: a public interface over an `internal` payload compiled
    // with zero diagnostics and then failed a consumer's `-D warnings` build,
    // and a public service could publish an `internal` interface. Each test
    // below pairs the refusal with the legal case it must leave alone; the
    // rule is about a *public* item naming a package-private one, and
    // rejecting `internal` over `internal` would be the worse failure.

    /// Every ridl type position is an exposure position. The fixture names one
    /// `internal` type from each of them at once — a signal, event and final
    /// payload, an array element, a command and query parameter, a stream
    /// element in both positions, a query return, a tuple-return field, and
    /// both arms of an inline `T | E` — so that a position quietly dropping out
    /// of the walk changes the count.
    #[test]
    fn typl_005_covers_every_interaction_type_position() {
        let checked = check_ridl(
            "app",
            "package app\n\
             type Tick : integer [0..100]\n\
             error struct Bang { code : Tick }\n\
             internal type Hidden : integer [0..10]\n\
             internal error struct Boom { code : Tick }\n\
             interface Panel {\n\
             \x20 signal a : Hidden @1s\n\
             \x20 event b : Hidden @[1s..2s]\n\
             \x20 final c : Hidden\n\
             \x20 final d : [Hidden; 1..4]\n\
             \x20 command e(p : Hidden)\n\
             \x20 command f(p : <Hidden>)\n\
             \x20 query g() : Hidden\n\
             \x20 query h() : (x : Hidden, y : Tick)\n\
             \x20 query i() : <Hidden>\n\
             \x20 query j() : Tick | Boom\n\
             \x20 query k() : Hidden | Bang\n\
             }\n",
        );
        assert_eq!(
            codes(&checked),
            vec!["TYPL-005"; 11],
            "one per exposure position; got: {:?}",
            messages(&checked),
        );
        assert!(
            checked.diagnostics[0].message.contains("public `Panel`")
                && checked.diagnostics[0].message.contains("`Hidden`"),
            "the message names both the exposing declaration and the exposed one: {}",
            checked.diagnostics[0].message,
        );
    }

    /// The legal direction, over the same eleven positions: an `internal`
    /// interface may name `internal` types freely. Both sides generate
    /// package-private code (ADR-0008 decision 7), so nothing is exposed —
    /// reporting here would be the over-rejection failure.
    #[test]
    fn internal_interface_may_name_internal_types_in_every_position() {
        let checked = check_ridl(
            "app",
            "package app\n\
             type Tick : integer [0..100]\n\
             error struct Bang { code : Tick }\n\
             internal type Hidden : integer [0..10]\n\
             internal error struct Boom { code : Tick }\n\
             internal const MAXLEN = 4\n\
             internal enum Mode { OFF = 0, ON = 1 }\n\
             internal interface Panel {\n\
             \x20 signal a : Hidden @1s\n\
             \x20 event b : Hidden @[1s..2s]\n\
             \x20 final c : Hidden\n\
             \x20 final d : [Hidden; 1..MAXLEN]\n\
             \x20 command e(p : Hidden)\n\
             \x20 command f(p : <Hidden>)\n\
             \x20 query g() : Hidden\n\
             \x20 query h() : (x : Hidden, y : Tick)\n\
             \x20 query i() : <Hidden>\n\
             \x20 query j() : Tick | Boom\n\
             \x20 query k() : Hidden | Bang\n\
             \x20 command l(p : Tick) [ require p < MAXLEN ]\n\
             \x20 query m(p : Tick) : Mode [ ensure result == Mode.ON ]\n\
             }\n",
        );
        assert!(codes(&checked).is_empty(), "got: {:?}", messages(&checked),);
    }

    /// A public interface over public types is clean even when the package
    /// holds `internal` declarations: the rule reads the referenced symbol's
    /// visibility, not the package's.
    #[test]
    fn public_interface_over_public_types_is_clean() {
        let checked = check_ridl(
            "app",
            "package app\n\
             type Tick : integer [0..100]\n\
             internal type Hidden : integer [0..10]\n\
             interface Panel {\n\
             \x20 signal a : Tick @1s\n\
             \x20 query g() : Tick\n\
             }\n",
        );
        assert!(codes(&checked).is_empty(), "got: {:?}", messages(&checked),);
    }

    /// A service's inline shape is an interface shape (ridl §14.5), and the
    /// service that holds it is always public — so its interaction types are
    /// exposure positions too. The named-interface half is covered by the
    /// interface tests above; this is the store a consumer walking
    /// `Package.interfaces` alone never sees.
    #[test]
    fn typl_005_reaches_a_service_inline_shape() {
        let checked = check_ridl(
            "app",
            "package app\n\
             internal type Hidden : integer [0..10]\n\
             service app.panel {\n\
             \x20 signal a : Hidden @1s\n\
             \x20 command e(p : Hidden)\n\
             }\n",
        );
        assert_eq!(codes(&checked), vec!["TYPL-005", "TYPL-005"]);
        assert!(
            checked.diagnostics[0].message.contains("`app.panel`"),
            "the message names the service: {}",
            checked.diagnostics[0].message,
        );
    }

    /// RIDL-143: a public service publishing an `internal` interface. It is not
    /// TYPL-005 because what leaks is an interface rather than a type, and
    /// because a service takes no `internal` modifier — so TYPL-005's other
    /// remedy, marking the exposing declaration internal too, does not exist.
    #[test]
    fn ridl_143_service_publishes_an_internal_interface() {
        let checked = check_ridl(
            "app",
            "package app\n\
             type Tick : integer [0..100]\n\
             internal interface Hidden {\n\
             \x20 signal a : Tick @1s\n\
             }\n\
             service app.panel : Hidden\n",
        );
        assert_eq!(codes(&checked), vec!["RIDL-143"]);
        let message = &checked.diagnostics[0].message;
        assert!(
            message.contains("`app.panel`") && message.contains("`Hidden`"),
            "the message names the service and the shape: {message}",
        );
        assert!(
            message.contains("inline shape"),
            "the message says what is allowed, not only what is refused: {message}",
        );
    }

    /// The legal counterpart: a service publishing a public interface. The
    /// shape's own payload types are public too, so nothing is exposed.
    #[test]
    fn service_publishing_a_public_interface_is_clean() {
        let checked = check_ridl(
            "app",
            "package app\n\
             type Tick : integer [0..100]\n\
             interface Shown {\n\
             \x20 signal a : Tick @1s\n\
             }\n\
             service app.panel : Shown\n",
        );
        assert!(codes(&checked).is_empty(), "got: {:?}", messages(&checked),);
    }

    /// A `require`/`ensure` clause is published verbatim: IR v2 carries its
    /// canonical source text (ADR-0008 decision 14) and both backends emit that
    /// text as data, so an `internal` constant or enum type named by one is an
    /// exposure exactly as a payload type is.
    #[test]
    fn typl_005_covers_a_contract_reference() {
        let checked = check_ridl(
            "app",
            "package app\n\
             type Tick : integer [0..100]\n\
             internal const SECRET_MAX = 7\n\
             internal enum Mode { OFF = 0, ON = 1 }\n\
             interface Panel {\n\
             \x20 command e(p : Tick) [ require p < SECRET_MAX ]\n\
             \x20 query g(p : Tick) : Tick [ ensure result > SECRET_MAX ]\n\
             \x20 command h(p : Mode) [ require p == Mode.ON ]\n\
             }\n",
        );
        // Two constant reads, then the parameter type and the enum head of
        // `Mode.ON` — the enum type is named twice, in two positions.
        assert_eq!(
            codes(&checked),
            vec!["TYPL-005"; 4],
            "got: {:?}",
            messages(&checked)
        );
        assert!(
            checked.diagnostics[0]
                .message
                .contains("internal constant `SECRET_MAX`"),
            "got: {}",
            checked.diagnostics[0].message,
        );
    }

    /// A parameter shadows a package constant of the same name — the contract
    /// environment binds parameters before the package vocabulary (expr-core
    /// §6), so the clause does not reference the constant and there is nothing
    /// to expose. Reporting here would reject a correct file.
    #[test]
    fn a_parameter_shadowing_an_internal_constant_is_not_an_exposure() {
        let checked = check_ridl(
            "app",
            "package app\n\
             type Tick : integer [0..100]\n\
             internal const level = 5\n\
             interface Panel {\n\
             \x20 command e(level : Tick) [ require level < 10 ]\n\
             }\n",
        );
        assert!(codes(&checked).is_empty(), "got: {:?}", messages(&checked),);
    }

    /// The two clause kinds do not bind the same names, and the exposure check
    /// has to follow that rather than assume it. A `require` sees the
    /// interface's own signals (ridl §13), so a signal spelled like an
    /// `internal` constant shadows it and the clause exposes nothing.
    ///
    /// An `ensure` does **not**: [`Checker::lower_contracts`] builds its scope
    /// with `signals: &[]`, so the same spelling resolves to the package
    /// constant and the published clause text names a declaration no importer
    /// can resolve. An earlier version of this check re-derived the binding
    /// order from the enclosing syntax, matched a `SignalDef` under the
    /// interface whatever the clause kind, and accepted the `ensure` case in
    /// silence. Both halves are asserted here so the pair cannot drift again.
    #[test]
    fn an_ensure_binds_no_signals_so_a_shadowing_signal_still_exposes() {
        let require = check_ridl(
            "app",
            "package app\n\
             type Tick : integer [0..100]\n\
             internal const MAX_LEVEL = 5\n\
             interface Panel {\n\
             \x20 signal MAX_LEVEL : Tick @1s\n\
             \x20 command c(p : Tick) [ require p < MAX_LEVEL ]\n\
             }\n",
        );
        assert!(
            codes(&require).is_empty(),
            "a `require` binds the signal, so nothing is exposed; got: {:?}",
            messages(&require),
        );

        let ensure = check_ridl(
            "app",
            "package app\n\
             type Tick : integer [0..100]\n\
             internal const MAX_LEVEL = 5\n\
             interface Panel {\n\
             \x20 signal MAX_LEVEL : Tick @1s\n\
             \x20 query g(p : Tick) : Tick [ ensure result > MAX_LEVEL ]\n\
             }\n",
        );
        assert_eq!(
            codes(&ensure),
            vec!["TYPL-005"],
            "an `ensure` binds no signal, so the name is the constant; got: {:?}",
            messages(&ensure),
        );
        assert!(
            ensure.diagnostics[0]
                .message
                .contains("internal constant `MAX_LEVEL`"),
            "got: {}",
            ensure.diagnostics[0].message,
        );
    }

    /// A collection length `Bound` is a bounds constant too (typl §3.3). It is
    /// a structurally distinct node from a scalar `Constraint` — a length bound
    /// is a direct child of the `ArrayType`/`MapType` — so the two had to be
    /// named separately, and only the constraint was, leaving two identical
    /// positions with one flagged and one silent. Both forms are asserted, and
    /// the `internal` counterpart must stay legal.
    #[test]
    fn typl_005_covers_a_collection_length_bound() {
        let typl = check_source(
            "app",
            "package app\n\
             type Tick : integer [0..100]\n\
             internal const MAXLEN = 4\n\
             struct Holder { f : [Tick; 1..MAXLEN] }\n",
        );
        assert_eq!(codes(&typl), vec!["TYPL-005"], "got: {:?}", messages(&typl));

        let ridl = check_ridl(
            "app",
            "package app\n\
             type Tick : integer [0..100]\n\
             internal const MAXLEN = 4\n\
             interface Panel {\n\
             \x20 final d : [Tick; 1..MAXLEN]\n\
             }\n",
        );
        assert_eq!(codes(&ridl), vec!["TYPL-005"], "got: {:?}", messages(&ridl));

        let legal = check_ridl(
            "app",
            "package app\n\
             type Tick : integer [0..100]\n\
             internal const MAXLEN = 4\n\
             internal interface Panel {\n\
             \x20 final d : [Tick; 1..MAXLEN]\n\
             }\n",
        );
        assert!(codes(&legal).is_empty(), "got: {:?}", messages(&legal));
    }

    /// A signal's `= value` override is not an exposure position, matching the
    /// typl rule the layer below applies: §3.3 names fields, arms, bounds
    /// constants and backing, and deliberately not init values, which resolve
    /// to a literal rather than carrying the constant's name.
    #[test]
    fn a_signal_init_override_is_not_an_exposure_position() {
        let checked = check_ridl(
            "app",
            "package app\n\
             type Tick : integer [0..100]\n\
             internal const SEED = 5\n\
             interface Panel {\n\
             \x20 signal a : Tick = SEED @1s\n\
             }\n",
        );
        assert!(codes(&checked).is_empty(), "got: {:?}", messages(&checked),);
    }

    // --- TYPL-404/405 and doc metadata (§14) ------------------------------

    #[test]
    fn deprecated_with_reason_populates_ir_without_warning() {
        let checked = check_source(
            "app",
            "package app\n/// @deprecated \"use Velocity\"\ntype Speed : km/h [0.0..250.0 step 0.5]\n",
        );
        assert!(codes(&checked).is_empty(), "got: {:?}", checked.diagnostics);
        assert_eq!(
            decl(&checked, "Speed").deprecated.as_deref(),
            Some("use Velocity"),
        );
    }

    #[test]
    fn typl_405_deprecated_without_reason() {
        let checked = check_source(
            "app",
            "package app\n/// @deprecated\ntype Speed : km/h [0.0..250.0 step 0.5]\n",
        );
        assert_eq!(codes(&checked), vec!["TYPL-405"]);
        assert_eq!(checked.diagnostics[0].severity, Severity::Warning);
        // The declaration is still marked deprecated, with an empty reason.
        assert_eq!(decl(&checked, "Speed").deprecated.as_deref(), Some(""));
    }

    #[test]
    fn typl_404_blank_line_between_doc_and_definition() {
        let checked = check_source(
            "app",
            "package app\n/// Vehicle speed\n\ntype Speed : km/h [0.0..250.0 step 0.5]\n",
        );
        assert_eq!(codes(&checked), vec!["TYPL-404"]);
        assert_eq!(checked.diagnostics[0].severity, Severity::Warning);
        // The doc still attaches despite the gap.
        assert_eq!(decl(&checked, "Speed").doc, "Vehicle speed");
    }

    #[test]
    fn labels_land_in_the_ir() {
        let checked = check_source(
            "app",
            "package app\n/// @labels SAFETY(D), CALIBRATION\ntype Speed : km/h [0.0..250.0 step 0.5]\n",
        );
        assert!(codes(&checked).is_empty(), "got: {:?}", checked.diagnostics);
        assert_eq!(
            decl(&checked, "Speed").labels,
            vec!["SAFETY(D)", "CALIBRATION"],
        );
    }

    #[test]
    fn doc_body_lands_in_the_ir() {
        let checked = check_source(
            "app",
            "package app\n/// Vehicle speed over ground\ntype Speed : km/h [0.0..250.0 step 0.5]\n",
        );
        assert_eq!(decl(&checked, "Speed").doc, "Vehicle speed over ground");
    }

    // --- TYPL-109: string/bytes init conformance --------------------------

    #[test]
    fn typl_109_string_init_too_long() {
        let checked = check_source(
            "app",
            "package app\ntype Tag : string [0..4] = \"toolong\"\n",
        );
        assert_eq!(codes(&checked), vec!["TYPL-109"]);
        assert!(checked.diagnostics[0].message.contains("length"));
    }

    #[test]
    fn typl_109_string_init_does_not_match_pattern() {
        let checked = check_source(
            "app",
            "package app\ntype Code : string [1..8 match /^[A-Z]+$/] = \"abc\"\n",
        );
        assert_eq!(codes(&checked), vec!["TYPL-109"]);
        assert!(checked.diagnostics[0].message.contains("pattern"));
    }

    #[test]
    fn conforming_string_init_is_clean() {
        let ok_len = check_source("app", "package app\ntype Tag : string [0..8] = \"hello\"\n");
        assert!(codes(&ok_len).is_empty(), "got: {:?}", ok_len.diagnostics);
        let ok_pattern = check_source(
            "app",
            "package app\ntype Code : string [1..8 match /^[A-Z]+$/] = \"ABC\"\n",
        );
        assert!(
            codes(&ok_pattern).is_empty(),
            "got: {:?}",
            ok_pattern.diagnostics,
        );
    }

    // --- E1.9: init derivation (§5.8) and TYPL-115 ------------------------

    /// A scalar `InitValue`.
    fn iv(derivable: bool, value: Option<&str>) -> v2::InitValue {
        v2::InitValue {
            derivable,
            value: value.map(str::to_string),
        }
    }

    /// The derived (or declared) init of struct `struct_name`'s field
    /// `field_name`.
    fn field_init(
        checked: &CheckedPackage,
        struct_name: &str,
        field_name: &str,
    ) -> Option<v2::InitValue> {
        struct_def(checked, struct_name)
            .members
            .iter()
            .find_map(|member| match member.member.as_ref()? {
                v2::struct_member::Member::Field(field) if field.name == field_name => {
                    Some(field.init.clone())
                }
                _ => None,
            })
            .flatten()
    }

    #[test]
    fn derived_boolean_init_is_false() {
        let checked = check_source("app", "package app\ntype Flag : boolean\n");
        assert!(codes(&checked).is_empty(), "got: {:?}", checked.diagnostics);
        assert_eq!(
            type_def(&checked, "Flag").init,
            Some(iv(true, Some("false")))
        );
    }

    #[test]
    fn derived_numeric_init_is_zero_in_range_else_min() {
        let in_range = check_source("app", "package app\ntype A : integer [0..20]\n");
        assert_eq!(type_def(&in_range, "A").init, Some(iv(true, Some("0"))));
        // `0` outside the range derives the minimum instead.
        let out = check_source("app", "package app\ntype B : integer [10..20]\n");
        assert_eq!(type_def(&out, "B").init, Some(iv(true, Some("10"))));
    }

    #[test]
    fn derived_string_init_is_empty_or_non_derivable() {
        // Bounds admit length 0: the empty string is the derived init.
        let empty = check_source("app", "package app\ntype S : string [0..8]\n");
        assert!(codes(&empty).is_empty(), "got: {:?}", empty.diagnostics);
        assert_eq!(type_def(&empty, "S").init, Some(iv(true, Some(""))));

        // A minimum length above 0 forbids the empty string: not derivable,
        // TYPL-115 (info).
        let bounded = check_source("app", "package app\ntype T : string [2..8]\n");
        assert_eq!(codes(&bounded), vec!["TYPL-115"]);
        assert_eq!(bounded.diagnostics[0].severity, Severity::Info);
        assert_eq!(type_def(&bounded, "T").init, Some(iv(false, None)));

        // A fixed-length bytes type is the same shape.
        let bytes = check_source("app", "package app\ntype H : bytes [32]\n");
        assert_eq!(codes(&bytes), vec!["TYPL-115"]);
        assert_eq!(type_def(&bytes, "H").init, Some(iv(false, None)));
    }

    #[test]
    fn match_typed_type_is_non_derivable_typl_115() {
        let checked = check_source(
            "app",
            "package app\ntype Code : string [1..8 match /^[A-Z]+$/]\n",
        );
        assert_eq!(codes(&checked), vec!["TYPL-115"]);
        assert_eq!(checked.diagnostics[0].severity, Severity::Info);
        assert_eq!(type_def(&checked, "Code").init, Some(iv(false, None)));
    }

    #[test]
    fn derived_enum_field_init_is_zero_if_declared_else_lowest() {
        let with_zero = check_source(
            "app",
            "package app\nenum E { A = 0, B = 1 }\nstruct S { e : E }\n",
        );
        assert_eq!(field_init(&with_zero, "S", "e"), Some(iv(true, Some("0"))));
        // No 0: the lowest declared value is the default.
        let without_zero = check_source(
            "app",
            "package app\nenum F { A = 5, B = 3 }\nstruct S { f : F }\n",
        );
        assert_eq!(
            field_init(&without_zero, "S", "f"),
            Some(iv(true, Some("3")))
        );
    }

    #[test]
    fn derived_enumset_field_init_is_empty() {
        let checked = check_source(
            "app",
            "package app\nenum W { A = 0, B = 1 }\nenumset Flags : W\nstruct S { fl : Flags }\n",
        );
        assert!(codes(&checked).is_empty(), "got: {:?}", checked.diagnostics);
        assert_eq!(field_init(&checked, "S", "fl"), Some(iv(true, Some(""))));
    }

    #[test]
    fn derived_struct_field_inits_recurse_with_optional_absent() {
        let checked = check_source(
            "app",
            "package app\n\
             type Speed: km/h [0.0..250.0 step 0.5]\n\
             struct S {\n\
               count : integer [0..10]\n\
               speed : Speed\n\
               over  : Speed?\n\
             }\n",
        );
        assert!(codes(&checked).is_empty(), "got: {:?}", checked.diagnostics);
        // An inline numeric field derives 0-in-range.
        assert_eq!(
            field_init(&checked, "S", "count"),
            Some(iv(true, Some("0")))
        );
        // A named numeric type field materializes the type's init.
        assert_eq!(
            field_init(&checked, "S", "speed"),
            Some(iv(true, Some("0")))
        );
        // An optional field is absent — derivable, no value.
        assert_eq!(field_init(&checked, "S", "over"), Some(iv(true, None)));
    }

    #[test]
    fn derived_union_field_init_reflects_first_arm() {
        // First arm derivable (Speed): the union field is a derivable composite.
        let derivable = check_source(
            "app",
            "package app\n\
             type Speed  : km/h [0.0..250.0 step 0.5]\n\
             type Counter: integer [0..255]\n\
             union U { fast : Speed, count : Counter }\n\
             struct S { u : U }\n",
        );
        assert_eq!(field_init(&derivable, "S", "u"), Some(iv(true, None)));

        // First arm non-derivable (a `match`-typed string): the union field is
        // not derivable.
        let non_derivable = check_source(
            "app",
            "package app\n\
             type Code   : string [1..8 match /^[A-Z]+$/]\n\
             type Counter: integer [0..255]\n\
             union V { code : Code, count : Counter }\n\
             struct S { v : V }\n",
        );
        assert_eq!(field_init(&non_derivable, "S", "v"), Some(iv(false, None)));
    }

    #[test]
    fn derived_tuple_field_init_is_a_derivable_composite() {
        let checked = check_source(
            "app",
            "package app\ntype Speed: km/h [0.0..250.0 step 0.5]\nstruct S { range : (min: Speed, max: Speed) }\n",
        );
        assert_eq!(field_init(&checked, "S", "range"), Some(iv(true, None)));
    }

    #[test]
    fn derived_collection_field_init_follows_the_min_count() {
        // A fixed array of a derivable element is derivable.
        let array = check_source(
            "app",
            "package app\ntype Speed: km/h [0.0..250.0 step 0.5]\nstruct S { xs : [Speed; 8] }\n",
        );
        assert_eq!(field_init(&array, "S", "xs"), Some(iv(true, None)));

        // A `min = 0` collection is empty, hence derivable regardless of its
        // (non-derivable) element types.
        let empty_map = check_source(
            "app",
            "package app\nstruct S { m : [Label : Name; 0..4] }\n",
        );
        assert_eq!(field_init(&empty_map, "S", "m"), Some(iv(true, None)));

        // A `min > 0` array of a non-derivable element (a `match`-typed
        // `ridl.std.Name`) is not derivable.
        let bounded = check_source("app", "package app\nstruct S { names : [Name; 1..4] }\n");
        assert_eq!(field_init(&bounded, "S", "names"), Some(iv(false, None)));
    }

    #[test]
    fn typl_109_declared_init_out_of_range_type_and_field() {
        // Type level (§5.8 declared init).
        let type_level = check_source(
            "app",
            "package app\ntype Speed: km/h [0.0..250.0 step 0.5] = 300.0\n",
        );
        assert_eq!(codes(&type_level), vec!["TYPL-109"]);
        // Field level — an inline numeric field init out of range.
        let field_level = check_source(
            "app",
            "package app\nstruct S { speed : integer [0..250] = 300 }\n",
        );
        assert_eq!(codes(&field_level), vec!["TYPL-109"]);
    }

    /// The T14 field-init obligation: a too-long string field init through a
    /// named string `type` now fires TYPL-109 (it passed silently before E1.9).
    #[test]
    fn typl_109_string_field_init_too_long_via_named_type() {
        let checked = check_source(
            "app",
            "package app\ntype Tag : string [0..4]\nstruct S { tag : Tag = \"toolong\" }\n",
        );
        assert_eq!(codes(&checked), vec!["TYPL-109"]);
        assert!(checked.diagnostics[0].message.contains("length"));
    }

    /// The inline-scalar analogue of the T14 obligation.
    #[test]
    fn typl_109_string_field_init_too_long_inline() {
        let checked = check_source(
            "app",
            "package app\nstruct S { tag : string [0..4] = \"toolong\" }\n",
        );
        assert_eq!(codes(&checked), vec!["TYPL-109"]);
        assert!(checked.diagnostics[0].message.contains("length"));
    }

    /// A field init that violates the named type's `match` pattern fires
    /// TYPL-109 (the `match`-typed `Code` itself is TYPL-115).
    #[test]
    fn typl_109_string_field_init_violates_named_pattern() {
        let checked = check_source(
            "app",
            "package app\ntype Code : string [1..8 match /^[A-Z]+$/]\nstruct S { code : Code = \"abc\" }\n",
        );
        assert!(
            codes(&checked).contains(&"TYPL-109"),
            "got: {:?}",
            checked.diagnostics,
        );
        assert!(
            checked
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("pattern")),
        );
    }

    #[test]
    fn conforming_string_field_init_via_named_type_is_clean() {
        let checked = check_source(
            "app",
            "package app\ntype Tag : string [0..8]\nstruct S { tag : Tag = \"hello\" }\n",
        );
        assert!(codes(&checked).is_empty(), "got: {:?}", checked.diagnostics);
    }

    /// The minor T14 obligation: a `const = const` init whose resolved numeric
    /// value violates the const's declared range is TYPL-108 (only direct
    /// numeric literals reached the check before E1.9).
    #[test]
    fn typl_108_constref_init_out_of_range() {
        let checked = check_source(
            "app",
            "package app\ntype Speed: km/h [0.0..250.0 step 0.5]\nconst BASE = 300.0\nconst FAST : Speed = BASE\n",
        );
        assert_eq!(codes(&checked), vec!["TYPL-108"]);
        assert!(checked.diagnostics[0].message.contains("FAST"));
    }

    // --- the E2.1b interaction structural pass ----------------------------

    /// A single-file workspace-member package whose one file is `.ridl`.
    fn ridl_package(db: &RidlDatabase, name: &str, text: &str) -> Package {
        let file = InputFile::new(
            db,
            format!("{}.ridl", name.replace('.', "/")),
            text.to_string(),
        );
        Package::new(
            db,
            name.to_string(),
            vec![file],
            PackageOrigin::WorkspaceMember,
            BTreeMap::new(),
            None,
        )
    }

    /// A single-file `.ridl` package carrying a configured `[defaults].timing`
    /// (the raw string the checker parses, ridl §9.1).
    fn ridl_package_with_default(
        db: &RidlDatabase,
        name: &str,
        text: &str,
        default_timing: &str,
    ) -> Package {
        let file = InputFile::new(
            db,
            format!("{}.ridl", name.replace('.', "/")),
            text.to_string(),
        );
        Package::new(
            db,
            name.to_string(),
            vec![file],
            PackageOrigin::WorkspaceMember,
            BTreeMap::new(),
            Some(default_timing.to_string()),
        )
    }

    /// Checks a single-package workspace whose one file is a `.ridl` file.
    fn check_ridl(name: &str, text: &str) -> CheckedPackage {
        let mut db = RidlDatabase::default();
        let std = std_package(&mut db);
        let pkg = ridl_package(&db, name, text);
        let ws = Workspace::new(&db, vec![pkg], BTreeMap::new());
        check_package(&db, ws, pkg, std)
    }

    /// The diagnostic messages, in order.
    fn messages(checked: &CheckedPackage) -> Vec<&str> {
        checked
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.as_str())
            .collect()
    }

    /// A clean vocabulary prefix for interaction tests.
    const PRELUDE: &str = "package app\ntype Speed: km/h [0.0..300.0 step 0.5]\n";

    #[test]
    fn ridl_104_return_type_on_command() {
        let bad = check_ridl(
            "app",
            &format!("{PRELUDE}interface I {{\n  command reset(): Speed\n}}\n"),
        );
        assert_eq!(codes(&bad), vec!["RIDL-104"]);

        let good = check_ridl(
            "app",
            &format!("{PRELUDE}interface I {{\n  command reset()\n}}\n"),
        );
        assert!(codes(&good).is_empty(), "got: {:?}", good.diagnostics);
    }

    #[test]
    fn ridl_105_query_returning_unit() {
        let bad = check_ridl(
            "app",
            &format!("{PRELUDE}interface I {{\n  query q(): ()\n}}\n"),
        );
        assert_eq!(codes(&bad), vec!["RIDL-105"]);

        let good = check_ridl(
            "app",
            &format!("{PRELUDE}interface I {{\n  query q(): Speed\n}}\n"),
        );
        assert!(codes(&good).is_empty(), "got: {:?}", good.diagnostics);
    }

    #[test]
    fn ridl_106_timing_or_attr_block_on_final() {
        // The timing half is `ridl_106_timing_on_every_kind_that_carries_none`;
        // this is the attribute-block half, which stays `final`-only because a
        // command and a query do take an attribute block (their contracts).
        let attributed = check_ridl(
            "app",
            &format!("{PRELUDE}interface I {{\n  final v : Version [ persist ]\n}}\n"),
        );
        assert_eq!(codes(&attributed), vec!["RIDL-106", "FORM-107"]);
        assert!(
            attributed.diagnostics[0]
                .message
                .starts_with("an attribute block is not valid on `final`"),
            "got: {}",
            attributed.diagnostics[0].message,
        );

        let good = check_ridl(
            "app",
            &format!("{PRELUDE}interface I {{\n  final v : Version\n}}\n"),
        );
        assert!(codes(&good).is_empty(), "got: {:?}", good.diagnostics);
    }

    /// RIDL-401 and RIDL-402 each carry a secondary label pointing at the
    /// declaration the reader has to look at — the tombstone that retired the
    /// name, and the declaration that wins a duplicate.
    ///
    /// RIDL-140 is the house precedent ("`x` is first declared here"), and it
    /// was the only code in the namespace using one. Without the label a reader
    /// of "`speed` is already declared in this body" has to search the body to
    /// find out which `speed` survives, which is exactly the fact the checker
    /// already knows and was throwing away.
    #[test]
    fn evolution_codes_point_at_the_declaration_they_are_about() {
        for (name, source, label) in [
            (
                "RIDL-401",
                format!(
                    "{PRELUDE}interface I {{\n  reserved legacyTemp\n  \
                     signal legacyTemp : Speed @10ms\n}}\n"
                ),
                "`legacyTemp` is retired here",
            ),
            (
                "RIDL-402",
                format!(
                    "{PRELUDE}interface I {{\n  signal speed : Speed @10ms\n  \
                     signal speed : Speed @10ms\n}}\n"
                ),
                "`speed` is declared here, and this is the one that is kept",
            ),
            (
                "RIDL-401, service inline shape",
                format!(
                    "{PRELUDE}service app.svc {{\n  reserved legacyTemp\n  \
                     signal legacyTemp : Speed @10ms\n}}\n"
                ),
                "`legacyTemp` is retired here",
            ),
            // Three declarations, not two. Two cannot distinguish "the label
            // points at the first" from "the label points at the previous
            // one": the map this reads was a `HashSet`, whose `insert` keeps
            // the existing entry, and `HashMap::insert` replaces it — so the
            // third declaration's label pointed at the *second*, which is
            // itself dropped, while the label claimed it was the one kept.
            (
                "RIDL-402, three declarations",
                format!(
                    "{PRELUDE}interface I {{\n  signal speed : Speed @10ms\n  \
                     signal speed : Speed @20ms\n  signal speed : Speed @30ms\n}}\n"
                ),
                "`speed` is declared here, and this is the one that is kept",
            ),
            // Two tombstones for one name: RIDL-401's label must stay on the
            // first, for the same reason.
            (
                "RIDL-401, two tombstones",
                format!(
                    "{PRELUDE}interface I {{\n  reserved legacyTemp\n  \
                     reserved legacyTemp\n  signal legacyTemp : Speed @10ms\n}}\n"
                ),
                "`legacyTemp` is retired here",
            ),
            // Both first-wins shapes again, inside a service's inline shape.
            // `lower_service_inline` is a hand-copied twin of the `interface`
            // loop, and the interface rows above cover neither of its two maps:
            // reverting either map in the twin alone left the whole workspace
            // green. The twin is where a later edit diverges unnoticed — #174
            // found a second instance of one bug two blocks below the first —
            // so each shape is provoked from both stores.
            (
                "RIDL-402, three declarations, service inline shape",
                format!(
                    "{PRELUDE}service app.svc {{\n  signal speed : Speed @10ms\n  \
                     signal speed : Speed @20ms\n  signal speed : Speed @30ms\n}}\n"
                ),
                "`speed` is declared here, and this is the one that is kept",
            ),
            (
                "RIDL-401, two tombstones, service inline shape",
                format!(
                    "{PRELUDE}service app.svc {{\n  reserved legacyTemp\n  \
                     reserved legacyTemp\n  signal legacyTemp : Speed @10ms\n}}\n"
                ),
                "`legacyTemp` is retired here",
            ),
        ] {
            let checked = check_ridl("app", &source);
            assert!(
                !checked.diagnostics.is_empty(),
                "{name} must draw a diagnostic",
            );
            // *Every* diagnostic of the run is checked, not the first: with
            // three declarations of one name there are two RIDL-402s, and it
            // was the second one whose label was wrong.
            for diagnostic in &checked.diagnostics {
                assert_eq!(
                    diagnostic.labels.len(),
                    1,
                    "{name} must carry one secondary label: {diagnostic:?}",
                );
                assert_eq!(diagnostic.labels[0].message, label, "{name}");
                // The label points at the *other* declaration, not back at the
                // primary span — a label on the same range says nothing.
                assert_ne!(
                    diagnostic.labels[0].span.range, diagnostic.primary.range,
                    "{name}: the label must point somewhere else",
                );
                let at = usize::from(diagnostic.labels[0].span.range.start());
                let earlier = &source[at..];
                assert!(
                    earlier.starts_with("legacyTemp") || earlier.starts_with("speed"),
                    "{name}: the label lands on the name it talks about, got `{}`",
                    &earlier[..earlier.len().min(20)],
                );
                // And it lands on the FIRST declaration of that name — the one
                // lowering keeps — so the name must not appear as a whole word
                // anywhere before it. Whole-word, not `contains`: a `reserved`
                // entry ends its line, so a space-delimited probe would miss
                // the two-tombstone case and the assertion would be vacuous
                // there.
                let bare: String = earlier
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                let earlier_mentions = source[..at]
                    .split(|c: char| !(c.is_alphanumeric() || c == '_'))
                    .filter(|word| *word == bare)
                    .count();
                assert_eq!(
                    earlier_mentions, 0,
                    "{name}: the label must point at the first declaration of `{bare}`, \
                     and {earlier_mentions} mention(s) already stand before it",
                );
            }
        }
    }

    /// The three-duplicate probe, kept as its own test because it is the shape
    /// the two-duplicate fixture cannot see: the label must name the
    /// declaration that survives lowering, and with three `signal speed` the
    /// survivor is the first while the previous-declaration answer is the
    /// second — itself dropped.
    #[test]
    fn a_third_duplicate_still_points_at_the_declaration_that_is_kept() {
        let source = format!(
            "{PRELUDE}interface I {{\n  signal speed : Speed @10ms\n  \
             signal speed : Speed @20ms\n  signal speed : Speed @30ms\n}}\n"
        );
        let checked = check_ridl("app", &source);
        assert_eq!(codes(&checked), vec!["RIDL-402", "RIDL-402"]);

        // One interaction survives, at ordinal 1 — the first declaration.
        assert_eq!(interface_walk(&checked), [("speed", 1)]);

        // Both labels point at that same first declaration, which is the only
        // `signal speed` with no earlier one.
        let first = checked.diagnostics[0].labels[0].span.range;
        assert_eq!(checked.diagnostics[1].labels[0].span.range, first);
        assert_eq!(
            source[..usize::from(first.start())]
                .matches("signal speed")
                .count(),
            0,
            "the label is on the first `signal speed`",
        );
        assert!(
            checked.diagnostics[1].primary.range.start() > first.start(),
            "the second RIDL-402 is raised after it",
        );
    }

    /// RIDL-107 is raised once, by the parser, and the checker adds nothing on
    /// top of it.
    ///
    /// It used to be raised twice for every stray declaration: the parser drew
    /// a FORM-102 "unexpected token in an interface body" and the checker then
    /// coded the recovered node RIDL-107, so one mistake produced two
    /// diagnostics at the same span whose messages contradicted each other.
    /// Asserting *both* halves is what pins that: the parse-side count alone
    /// would pass if the checker started re-reporting, and the checker-side
    /// silence alone would pass if the parser went back to FORM-102.
    #[test]
    fn ridl_107_is_raised_once_by_the_parser() {
        for (shape, source) in [
            (
                "an interface",
                format!("{PRELUDE}interface I {{\n  type X: m\n  signal s : Speed @10ms\n}}\n"),
            ),
            (
                "an interface, composite",
                format!("{PRELUDE}interface I {{\n  struct S {{ a: Speed }}\n}}\n"),
            ),
            (
                "a service inline shape",
                format!("{PRELUDE}service app.svc {{\n  type X: m\n  signal s : Speed @10ms\n}}\n"),
            ),
        ] {
            let parsed = ridl_syntax::parse(&source, ridl_syntax::Profile::Ridl);
            let raised: Vec<&str> = parsed.errors().iter().map(|error| error.code).collect();
            assert_eq!(raised, vec!["RIDL-107"], "{shape}: {:?}", parsed.errors());
            let message = &parsed.errors()[0].message;
            assert!(
                message.contains("move the declaration to package level"),
                "{shape}: the message must say where the declaration goes: {message}",
            );

            let checked = check_ridl("app", &source);
            assert!(
                !codes(&checked).contains(&"RIDL-107"),
                "{shape}: the checker must not re-report the parser's code: {:?}",
                checked.diagnostics,
            );
        }
    }

    #[test]
    fn interface_members_survive_an_in_body_composite_declaration() {
        // The T5 review fix: the in-body recovery is brace-aware, so a
        // composite declaration's own `}` (and an enum's commas) never close
        // the interface — the members after it keep their place and their
        // ordinals, and each stray declaration draws its own RIDL-107.
        let text = format!(
            "{PRELUDE}interface I {{\n  struct Extra {{ a: Speed }}\n  signal s : Speed @10ms\n  enum Mode {{ A, B }}\n  event e : Speed\n}}\n"
        );
        let checked = check_ridl("app", &text);
        // The two in-body composites are the parser's RIDL-107 (see
        // `ridl_107_is_raised_once_by_the_parser`), so the only checker
        // diagnostic left here is RIDL-100 for the untimed `event e`.
        assert_eq!(codes(&checked), vec!["RIDL-100"]);
        assert_eq!(
            ridl_syntax::parse(&text, ridl_syntax::Profile::Ridl)
                .errors()
                .iter()
                .map(|error| error.code)
                .collect::<Vec<_>>(),
            vec!["RIDL-107", "RIDL-107"],
            "each stray declaration draws its own code",
        );

        assert_eq!(interface_walk(&checked), [("s", 1), ("e", 2)]);
    }

    #[test]
    fn unclosed_interface_keeps_the_following_declarations() {
        // The T5 review fix, second manifestation: a genuinely unclosed `{`
        // reports FORM-103 at parse and hands the following declarations —
        // brace-carrying ones included — back to the package level, so their
        // symbols stay and nothing cascades.
        let mut db = RidlDatabase::default();
        let std = std_package(&mut db);
        let pkg = ridl_package(
            &db,
            "app",
            "package app\ntype Speed: km/h [0.0..300.0 step 0.5]\ninterface I {\n  signal s : Speed @10ms\n\ntype After : integer [0..1]\n\nstruct Uses { f: After }\n",
        );
        let ws = Workspace::new(&db, vec![pkg], BTreeMap::new());
        let resolution = resolve_package(&db, ws, pkg, std);
        assert!(resolution.symbols.contains_key("After"), "`After` survives");
        assert!(resolution.symbols.contains_key("Uses"), "`Uses` survives");
        let checked = check_package(&db, ws, pkg, std);
        assert!(codes(&checked).is_empty(), "got: {:?}", checked.diagnostics);
    }

    #[test]
    fn ridl_109_signal_payload_without_derivable_init() {
        // `string [11..17]` has no derivable init (len_min > 0, typl §5.8).
        let bad = check_ridl(
            "app",
            &format!(
                "{PRELUDE}type Plate: string [11..17]\ninterface I {{\n  signal plate : Plate @10ms\n}}\n"
            ),
        );
        assert!(
            codes(&bad).contains(&"RIDL-109"),
            "got: {:?}",
            bad.diagnostics
        );
        let ridl_109 = bad
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code.as_str() == "RIDL-109")
            .expect("checked above");
        assert_eq!(ridl_109.severity, Severity::Error);

        // A conforming `= value` override satisfies §4.4 — no RIDL-109/110.
        let good = check_ridl(
            "app",
            &format!(
                "{PRELUDE}type Plate: string [11..17]\ninterface I {{\n  signal plate : Plate = \"AAAAAAAAAAA\" @10ms\n}}\n"
            ),
        );
        assert!(
            !codes(&good).contains(&"RIDL-109"),
            "got: {:?}",
            good.diagnostics
        );
        assert!(
            !codes(&good).contains(&"RIDL-110"),
            "got: {:?}",
            good.diagnostics
        );
    }

    #[test]
    fn ridl_110_signal_init_override_violating_the_payload_constraints() {
        let bad = check_ridl(
            "app",
            &format!("{PRELUDE}interface I {{\n  signal s : Speed = 400.0 @10ms\n}}\n"),
        );
        assert_eq!(codes(&bad), vec!["RIDL-110"]);
        assert!(
            bad.diagnostics[0]
                .message
                .contains("outside the declared range"),
            "got: {}",
            bad.diagnostics[0].message,
        );

        // A SCREAMING_SNAKE constant reference resolves through the E1 const
        // chain and validates in range.
        let good = check_ridl(
            "app",
            &format!(
                "{PRELUDE}const CRUISE = 120.0\ninterface I {{\n  signal s : Speed = CRUISE @10ms\n}}\n"
            ),
        );
        assert!(codes(&good).is_empty(), "got: {:?}", good.diagnostics);

        let bad_const = check_ridl(
            "app",
            &format!(
                "{PRELUDE}const OVER = 400.0\ninterface I {{\n  signal s : Speed = OVER @10ms\n}}\n"
            ),
        );
        assert_eq!(codes(&bad_const), vec!["RIDL-110"]);
    }

    #[test]
    fn ridl_201_stream_payload_on_signal_or_event() {
        let checked = check_ridl(
            "app",
            &format!("{PRELUDE}interface I {{\n  signal s : <Speed>\n  event e : <Speed>\n}}\n"),
        );
        // Both are untimed, so each draws RIDL-100 (default applied) when it
        // lowers, before the stream payloads draw RIDL-201 (E2 task 9).
        assert_eq!(
            codes(&checked),
            vec!["RIDL-100", "RIDL-100", "RIDL-201", "RIDL-201"],
        );
    }

    #[test]
    fn ridl_202_stream_element_not_a_named_type() {
        let bad = check_ridl(
            "app",
            &format!("{PRELUDE}interface I {{\n  query q(): <integer>\n}}\n"),
        );
        assert_eq!(codes(&bad), vec!["RIDL-202"]);

        // Named elements and the raw `string`/`bytes` exception are legal
        // (ridl §12.2).
        let good = check_ridl(
            "app",
            &format!(
                "{PRELUDE}interface I {{\n  query named(): <Speed>\n  query raw(): <string>\n}}\n"
            ),
        );
        assert!(codes(&good).is_empty(), "got: {:?}", good.diagnostics);
    }

    #[test]
    fn ridl_301_contracts_on_signal_event_final() {
        let on_signal = check_ridl(
            "app",
            &format!("{PRELUDE}interface I {{\n  signal s : Speed @10ms [ require s > 0.0 ]\n}}\n"),
        );
        assert_eq!(codes(&on_signal), vec!["RIDL-301"]);

        let on_event = check_ridl(
            "app",
            &format!("{PRELUDE}interface I {{\n  event e : Speed [ ensure x > 0.0 ]\n}}\n"),
        );
        // The untimed event also draws RIDL-100 (default applied, E2 task 9).
        assert_eq!(codes(&on_event), vec!["RIDL-301", "RIDL-100"]);

        // On a `final` the block itself is already RIDL-106; the predicate
        // additionally draws RIDL-301 (ridl §16.3 lists `final`).
        let on_final = check_ridl(
            "app",
            &format!("{PRELUDE}interface I {{\n  final v : Version [ require x > 0.0 ]\n}}\n"),
        );
        assert_eq!(codes(&on_final), vec!["RIDL-106", "RIDL-301"]);

        // `require` on command/query and `ensure` on query are the legal
        // homes (ridl §13).
        let good = check_ridl(
            "app",
            &format!(
                "{PRELUDE}interface I {{\n  command c(p: Speed) [ require p > 0.0 ]\n  query q(w: Speed): Speed [ require w > 0.0\n    ensure result >= 0.0 ]\n}}\n"
            ),
        );
        assert!(codes(&good).is_empty(), "got: {:?}", good.diagnostics);
    }

    #[test]
    fn ridl_302_ensure_on_command() {
        let checked = check_ridl(
            "app",
            &format!("{PRELUDE}interface I {{\n  command c() [ ensure x > 0.0 ]\n}}\n"),
        );
        assert_eq!(codes(&checked), vec!["RIDL-302"]);
    }

    /// A vocabulary rich enough for the contract cases: the `PRELUDE` speed
    /// type plus a second named numeric type, an enum, and a constant.
    const CONTRACT_PRELUDE: &str = "package app\n\
type Speed  : km/h [0.0..300.0 step 0.5]\n\
type Torque : N.m  [0.0..900.0 step 0.5]\n\
type Count  : integer [0..1000]\n\
type Ratio  : float [0.0..1.0 step 0.1]\n\
const MAX_SPEED : Speed = 250.0\n\
const MAX_COUNT : Count = 500\n\
enum GearPosition {\n  PARK  = 0\n  DRIVE = 1\n}\n";

    /// The expr-core §8 boundary, end to end through the package checker, each
    /// row pinned by the **rule that fires** and the source it fires on.
    ///
    /// RIDL-306 covers the whole boundary with one code, so `codes ==
    /// ["RIDL-306"]` is satisfied by any rule at all rejecting the row — the
    /// wrong one included. The `expr.rs` counterpart of this test carries the
    /// worked demonstration; the same reasoning applies here, over the checker
    /// that assembles the scope rather than over the typing rules alone.
    #[test]
    fn ridl_306_expression_outside_the_guaranteed_subset() {
        // `(name, body, message fragment, the source the span must cover)`.
        for (name, body, fragment, offending) in [
            (
                "unknown reference",
                "command c(p: Speed) [ require unknownName > 0 ]",
                "`unknownName` does not resolve here",
                "unknownName",
            ),
            (
                "cross-domain arithmetic",
                "command c(speed: Speed, window: Duration) [ require speed + window > 0 ]",
                "`+` over a duration",
                "speed + window",
            ),
            (
                "cross-named-type arithmetic",
                "command c(speed: Speed, torque: Torque) [ require speed + torque > 0.0 ]",
                "`+` requires numeric operands of one type",
                "speed + torque",
            ),
            (
                "non-boolean root",
                "command c(p: Speed) [ require 3 ]",
                "not a predicate",
                "3",
            ),
            (
                "signal read in an ensure",
                "query q(): Speed [ ensure currentSpeed >= 0.0 ]",
                "`currentSpeed` does not resolve here",
                "currentSpeed",
            ),
            (
                "a qualified member chain",
                "command c(position: GearPosition) [ require position != app.GearPosition.PARK ]",
                "`.GearPosition` names a type, not a member",
                "app.GearPosition",
            ),
            (
                "`result` in a require",
                "query q(): Speed [ require result >= 0.0 ]",
                "`result` is not in scope here",
                "result",
            ),
            (
                "struct-field access",
                "query q(n: Count): Speed [ require n.severity >= 4\n    ensure result >= 0.0 ]",
                "the guaranteed subset admits field access on a tuple-typed `result` only",
                "n.severity",
            ),
            (
                "`%` over a float-backed operand",
                "query q(n: Count): Speed [ require n % 0.5 == 0.0\n    ensure result >= 0.0 ]",
                "`%` requires integer-backed operands",
                "n % 0.5",
            ),
            (
                "duration arithmetic",
                "query q(w: Duration): Speed [ require w + 10ms < 1s\n    ensure result >= 0.0 ]",
                "`+` over a duration",
                "w + 10ms",
            ),
        ] {
            let source = format!(
                "{CONTRACT_PRELUDE}interface I {{\n  signal currentSpeed : Speed @10ms\n  {body}\n}}\n"
            );
            let checked = check_ridl("app", &source);
            assert_eq!(
                codes(&checked),
                vec!["RIDL-306"],
                "{name}: {:?}",
                checked.diagnostics
            );
            let diagnostic = &checked.diagnostics[0];
            assert!(
                diagnostic.message.contains(fragment),
                "{name} must be rejected by the rule that says `{fragment}`, got: {}",
                diagnostic.message,
            );
            let range = diagnostic.primary.range;
            assert_eq!(
                &source[usize::from(range.start())..usize::from(range.end())],
                offending,
                "{name}: the span must cover the form the rule rejected",
            );
        }
    }

    #[test]
    fn constants_unify_nominally_by_their_declared_type() {
        // A constant carries its declared type, so a cross-named-type
        // comparison against it is as much an error as any other
        // (expr-core §5.2 — two named types never unify; typl §5.7).
        for (name, body, both) in [
            (
                "float-backed named type against a `Speed` constant",
                "command c(t: Torque) [ require t < MAX_SPEED ]",
                ["`app.Torque`", "`app.Speed`"],
            ),
            (
                "integer-backed named type against a `Speed` parameter",
                "command c(s: Speed) [ require s < MAX_COUNT ]",
                ["`app.Speed`", "`app.Count`"],
            ),
        ] {
            let checked = check_ridl(
                "app",
                &format!("{CONTRACT_PRELUDE}interface I {{\n  {body}\n}}\n"),
            );
            assert_eq!(
                codes(&checked),
                vec!["RIDL-306"],
                "{name}: {:?}",
                checked.diagnostics
            );
            // The rule is nominal unification, and the evidence is that the
            // message names *both* types: "requires operands of one ordered
            // domain, found X and Y". A message naming one of them would be
            // some other rule reaching the same code.
            let message = &checked.diagnostics[0].message;
            assert!(
                message.contains("`<` requires operands of one ordered domain"),
                "{name}: {message}",
            );
            for named in both {
                assert!(
                    message.contains(named),
                    "{name}: the message must name {named}: {message}",
                );
            }
        }

        // The expr-core §9 walk of `require max <= MAX_SPEED`: a `Speed`
        // parameter against a `Speed` constant, ordered over one named type.
        let checked = check_ridl(
            "app",
            &format!(
                "{CONTRACT_PRELUDE}interface I {{\n  command c(max: Speed) [ require max <= MAX_SPEED ]\n}}\n"
            ),
        );
        assert!(codes(&checked).is_empty(), "got: {:?}", checked.diagnostics);
    }

    #[test]
    fn remainder_reads_the_operand_backing_not_the_literal_spelling() {
        // `%` is integer-backed only (expr-core §5.3). The rule is about the
        // operands' declared types, so a float-backed named type is rejected
        // even where every literal in sight is written as an integer.
        for (name, body) in [
            (
                "two float-backed named types",
                "command c(a: Speed, b: Speed) [ require a % b == 0.0 ]",
            ),
            (
                "a float-backed constant",
                "command c(a: Speed) [ require a % MAX_SPEED == 0.0 ]",
            ),
            (
                "a bare float-backed type",
                "command c(a: Ratio, b: Ratio) [ require a % b == 0.0 ]",
            ),
            (
                "an integer-backed type against a float literal",
                "command c(a: Count) [ require a % 0.5 == 0 ]",
            ),
        ] {
            let checked = check_ridl(
                "app",
                &format!("{CONTRACT_PRELUDE}interface I {{\n  {body}\n}}\n"),
            );
            assert_eq!(
                codes(&checked),
                vec!["RIDL-306"],
                "{name}: {:?}",
                checked.diagnostics
            );
            // It must be the `%` backing rule that fires. Every row here is
            // well-formed apart from the backing, so any other RIDL-306 rule
            // reaching this code would be reporting the wrong thing.
            assert!(
                checked.diagnostics[0]
                    .message
                    .contains("`%` requires integer-backed operands"),
                "{name}: {}",
                checked.diagnostics[0].message,
            );
        }

        // Integer-backed operands, named and literal, are the legal form.
        let checked = check_ridl(
            "app",
            &format!(
                "{CONTRACT_PRELUDE}interface I {{\n  command c(a: Count, b: Count) [\n    require a % b == 0\n    require a % MAX_COUNT == 0\n  ]\n}}\n"
            ),
        );
        assert!(codes(&checked).is_empty(), "got: {:?}", checked.diagnostics);
    }

    #[test]
    fn ridl_306_unknown_enum_member() {
        // An unknown member is as broken a reference as an unknown identifier:
        // the task 12 observers and the E2.11 property runner have no value to
        // bind to it.
        let checked = check_ridl(
            "app",
            &format!(
                "{CONTRACT_PRELUDE}interface I {{\n  command c(p: GearPosition) [ require p != GearPosition.TYPO ]\n}}\n"
            ),
        );
        assert_eq!(codes(&checked), vec!["RIDL-306"]);
        assert!(
            checked.diagnostics[0].message.contains("TYPO"),
            "the message names the member: {}",
            checked.diagnostics[0].message
        );

        // A declared member is accepted.
        let good = check_ridl(
            "app",
            &format!(
                "{CONTRACT_PRELUDE}interface I {{\n  command c(p: GearPosition) [ require p != GearPosition.PARK ]\n}}\n"
            ),
        );
        assert!(codes(&good).is_empty(), "got: {:?}", good.diagnostics);
    }

    #[test]
    fn out_of_domain_parameter_reports_its_type_not_its_resolution() {
        // `l` IS a parameter — what is outside the subset is its type — so the
        // message must name the declared form (expr-core §8).
        let checked = check_ridl(
            "app",
            &format!(
                "{CONTRACT_PRELUDE}interface I {{\n  command c(l: Label) [ require l == \"x\" ]\n}}\n"
            ),
        );
        assert!(
            codes(&checked).iter().all(|code| *code == "RIDL-306"),
            "got: {:?}",
            checked.diagnostics
        );
        let message = &checked.diagnostics[0].message;
        assert!(
            message.contains("parameter") && message.contains("ridl.std.Label"),
            "the message names the parameter and its type: {message}"
        );
        assert!(
            !message.contains("does not resolve"),
            "the parameter resolves; its type is the problem: {message}"
        );
    }

    #[test]
    fn respelling_a_literal_does_not_change_contract_source() {
        // A respelled literal is the same exact rational (expr-core §7), so it
        // must not read as a contract edit in `ridl diff`.
        let sources = |literal: &str| {
            let checked = check_ridl(
                "app",
                &format!(
                    "{CONTRACT_PRELUDE}interface I {{\n  query q(): Speed [ ensure result >= {literal} ]\n}}\n"
                ),
            );
            assert!(codes(&checked).is_empty(), "got: {:?}", checked.diagnostics);
            query_def(&checked, "q").contracts[0].source.clone()
        };
        assert_eq!(sources("0.0"), "result >= 0.0");
        assert_eq!(sources("0.00"), sources("0.0"));
        assert_eq!(
            sources("250.0"),
            "result >= 250.0",
            "a whole float keeps its form"
        );
        assert_eq!(sources("1.50"), "result >= 1.5");
    }

    #[test]
    fn an_imported_constant_carries_the_type_of_its_own_package() {
        // The constant's type is resolved in the package that declares it, so
        // the checked package does not have to import `Speed` to compare a
        // `Speed` value against `MAX_SPEED`.
        let mut db = RidlDatabase::default();
        let std = std_package(&mut db);
        let common = package(
            &db,
            "veh.common",
            "package veh.common\n\
type Speed : km/h [0.0..300.0 step 0.5]\n\
type Torque : N.m [0.0..900.0 step 0.5]\n\
const MAX_SPEED : Speed = 250.0\n",
        );
        let cluster = ridl_package(
            &db,
            "veh.cluster",
            "package veh.cluster\n\
import veh.common.Speed\n\
import veh.common.Torque\n\
import veh.common.MAX_SPEED\n\
interface I {\n\
  command ok(s: Speed) [ require s <= MAX_SPEED ]\n\
  command bad(t: Torque) [ require t <= MAX_SPEED ]\n\
}\n",
        );
        let ws = Workspace::new(&db, vec![cluster, common], BTreeMap::new());
        let checked = check_package(&db, ws, cluster, std);
        assert_eq!(
            codes(&checked),
            vec!["RIDL-306"],
            "only the `Torque` comparison is a type error: {:?}",
            checked.diagnostics
        );
        assert!(
            checked.diagnostics[0].message.contains("veh.common.Torque")
                && checked.diagnostics[0].message.contains("veh.common.Speed"),
            "the message names both named types: {}",
            checked.diagnostics[0].message
        );
    }

    #[test]
    fn ridl_305_ensure_that_never_reads_result() {
        let checked = check_ridl(
            "app",
            &format!(
                "{CONTRACT_PRELUDE}interface I {{\n  query q(window: Duration): Speed [ ensure window > 0ms ]\n}}\n"
            ),
        );
        assert_eq!(codes(&checked), vec!["RIDL-305"]);
        assert_eq!(checked.diagnostics[0].severity, Severity::Warning);
    }

    #[test]
    fn guaranteed_subset_forms_check_clean() {
        // The ridl §13 guaranteed forms, including the tuple-field access and
        // the enum-access form, over one interface.
        let checked = check_ridl(
            "app",
            &format!(
                "{CONTRACT_PRELUDE}interface I {{\n\
  signal currentSpeed : Speed @10ms\n\
  command setRange(min: Speed, max: Speed) [\n\
    require min < max\n\
    require max <= MAX_SPEED\n\
  ]\n\
  command setGear(position: GearPosition) [\n\
    require position != GearPosition.PARK || currentSpeed == 0.0\n\
  ]\n\
  query getAverageSpeed(window: Duration): Speed [\n\
    require window > 0ms\n\
    ensure result >= 0.0\n\
  ]\n\
  query getRange(): (min: Speed, max: Speed) [\n\
    ensure result.min >= 0.0 && result.max <= MAX_SPEED\n\
  ]\n\
  query stepsOf(n: Count): Count [\n\
    require n % 2 == 0\n\
    ensure result >= 0\n\
  ]\n\
}}\n"
            ),
        );
        assert!(codes(&checked).is_empty(), "got: {:?}", checked.diagnostics);
    }

    #[test]
    fn contract_source_lowers_canonical_text() {
        // ADR-0008 decision 14: the IR carries the canonical rendering, not
        // the written spacing, so reformatting a file is not a contract edit.
        let checked = check_ridl(
            "app",
            &format!(
                "{CONTRACT_PRELUDE}interface I {{\n\
  query q(min: Speed, max: Speed): Speed [\n\
    require (min<max)&&(max<=MAX_SPEED)\n\
    ensure  result>=0.0\n\
  ]\n\
}}\n"
            ),
        );
        assert!(codes(&checked).is_empty(), "got: {:?}", checked.diagnostics);
        let sources: Vec<&str> = query_def(&checked, "q")
            .contracts
            .iter()
            .map(|contract| contract.source.as_str())
            .collect();
        assert_eq!(sources, ["min < max && max <= MAX_SPEED", "result >= 0.0"],);
    }

    /// The `(observer_id, signal_refs, param_refs, uses_result)` stub of every
    /// contract on the interaction named `name`, in lowering order.
    fn observer_stubs<'a>(
        checked: &'a CheckedPackage,
        name: &str,
    ) -> Vec<(&'a str, Vec<&'a str>, Vec<&'a str>, bool)> {
        let contracts = match &interaction(checked, name).kind {
            Some(v2::decl::Kind::CommandDef(command)) => &command.contracts,
            Some(v2::decl::Kind::QueryDef(query)) => &query.contracts,
            _ => panic!("`{name}` carries no contracts"),
        };
        contracts
            .iter()
            .map(|contract| {
                (
                    contract.observer_id.as_str(),
                    contract
                        .signal_refs
                        .iter()
                        .map(String::as_str)
                        .collect::<Vec<_>>(),
                    contract
                        .param_refs
                        .iter()
                        .map(String::as_str)
                        .collect::<Vec<_>>(),
                    contract.uses_result,
                )
            })
            .collect()
    }

    #[test]
    fn contract_observer_ids_number_within_the_clause_kind() {
        // Task 12: `i` counts the interaction's clauses **of that kind**, so
        // two requires on one command are `[0]` and `[1]`.
        let checked = check_ridl(
            "app",
            &format!(
                "{CONTRACT_PRELUDE}interface I {{\n\
  command c(min: Speed, max: Speed) [\n\
    require min < max\n\
    require max <= MAX_SPEED\n\
  ]\n\
}}\n"
            ),
        );
        assert!(codes(&checked).is_empty(), "got: {:?}", checked.diagnostics);
        assert_eq!(
            observer_stubs(&checked, "c"),
            [
                ("I.c.require[0]", vec![], vec!["min", "max"], false),
                ("I.c.require[1]", vec![], vec!["max"], false),
            ],
        );
    }

    #[test]
    fn appending_a_clause_does_not_renumber_the_earlier_observer_ids() {
        // The observer id is the handle E5/E7 observer tooling is expected to
        // address a single clause by, so appending a clause must leave every
        // earlier id untouched — which is why `i` is scoped to the clause kind
        // rather than counted across kinds. (Removal is the known limitation,
        // pinned by the test below.)
        let one_require = check_ridl(
            "app",
            &format!(
                "{CONTRACT_PRELUDE}interface I {{\n\
  query q(w: Duration): Speed [\n\
    require w > 0ms\n\
  ]\n\
}}\n"
            ),
        );
        assert_eq!(
            observer_stubs(&one_require, "q")[0].0,
            "I.q.require[0]",
            "the sole require is `[0]`",
        );

        // A second require appended after it: the first keeps `[0]`.
        let two_requires = check_ridl(
            "app",
            &format!(
                "{CONTRACT_PRELUDE}interface I {{\n\
  query q(w: Duration): Speed [\n\
    require w > 0ms\n\
    require w < 10s\n\
  ]\n\
}}\n"
            ),
        );
        let ids: Vec<&str> = observer_stubs(&two_requires, "q")
            .iter()
            .map(|stub| stub.0)
            .collect();
        assert_eq!(ids, ["I.q.require[0]", "I.q.require[1]"]);

        // An `ensure` added beside the require: the require still reads `[0]`
        // and the ensure opens its own kind-scoped run at `[0]`.
        let require_and_ensure = check_ridl(
            "app",
            &format!(
                "{CONTRACT_PRELUDE}interface I {{\n\
  query q(w: Duration): Speed [\n\
    require w > 0ms\n\
    ensure  result >= 0.0\n\
  ]\n\
}}\n"
            ),
        );
        let ids: Vec<&str> = observer_stubs(&require_and_ensure, "q")
            .iter()
            .map(|stub| stub.0)
            .collect();
        assert_eq!(ids, ["I.q.require[0]", "I.q.ensure[0]"]);
    }

    #[test]
    fn removing_a_clause_renumbers_the_survivors_known_limitation() {
        // The index is **positional**: it is stable under append (the test
        // above) but a clause *removal* shifts every later clause of the same
        // kind down one, so a surviving id starts addressing a different
        // predicate. Deleting the first of two requires moves the survivor
        // from `[1]` to `[0]`.
        //
        // This is known and accepted behavior, not an oversight — pinned here
        // so it can never be re-discovered as a regression. A tombstone or
        // explicit-index mechanism is recorded against ADR-0008 for E5/E7.
        let both = check_ridl(
            "app",
            &format!(
                "{CONTRACT_PRELUDE}interface I {{\n\
  command c(min: Speed, max: Speed) [\n\
    require min < max\n\
    require max <= MAX_SPEED\n\
  ]\n\
}}\n"
            ),
        );
        let stubs = observer_stubs(&both, "c");
        let survivor = &stubs[1];
        assert_eq!(
            (survivor.0, &survivor.2),
            ("I.c.require[1]", &vec!["max"]),
            "with both clauses present, `max <= MAX_SPEED` is `[1]`",
        );

        // The first require deleted: the same surviving predicate now answers
        // to `[0]`.
        let first_removed = check_ridl(
            "app",
            &format!(
                "{CONTRACT_PRELUDE}interface I {{\n\
  command c(min: Speed, max: Speed) [\n\
    require max <= MAX_SPEED\n\
  ]\n\
}}\n"
            ),
        );
        let stubs = observer_stubs(&first_removed, "c");
        let survivor = &stubs[0];
        assert_eq!(
            (survivor.0, &survivor.2),
            ("I.c.require[0]", &vec!["max"]),
            "the survivor renumbers to `[0]` — the accepted limitation",
        );
    }

    #[test]
    fn ridl_401_interaction_redeclared_under_a_reserved_name() {
        let checked = check_ridl(
            "app",
            &format!(
                "{PRELUDE}interface I {{\n  reserved resetCounters\n  query resetCounters(w: Speed): Speed\n}}\n"
            ),
        );
        // `resetCounters` is also mutation-named, so the E2.10a lint fires
        // alongside: the two rules are independent and both hold here.
        assert_eq!(codes(&checked), vec!["RIDL-401", "RIDL-404"]);
    }

    #[test]
    fn ridl_402_duplicate_interaction_name_first_wins() {
        let checked = check_ridl(
            "app",
            &format!(
                "{PRELUDE}interface I {{\n  signal a : Speed @10ms\n  event a : Speed\n  query b(): Speed\n}}\n"
            ),
        );
        assert_eq!(codes(&checked), vec!["RIDL-402"]);

        // The loser is excluded and holds no ordinal slot — lowering keeps
        // exactly the first declaration. What the ordinals then are, and which
        // kind survives, is `ridl_402_duplicate_lowers_first_wins_into_the_ir`.
        assert_eq!(interface_walk(&checked), [("a", 1), ("b", 2)]);
    }

    /// One rule, one code: a timing annotation on any kind that carries none is
    /// RIDL-106, and each message says why *that* kind carries none.
    ///
    /// `command` and `query` used to draw FORM-102 while `final` drew RIDL-106.
    /// FORM-102's catalogue meaning is "unexpected token", and the grammar
    /// accepts `@` on all five kinds on purpose, so the token was never
    /// unexpected — the rejection is semantic and now wears a semantic code.
    #[test]
    fn ridl_106_timing_on_every_kind_that_carries_none() {
        for (kind, member, because) in [
            ("command", "command c() @10ms", "invoked on demand"),
            ("query", "query q(): Speed @10ms", "answered on demand"),
            ("final", "final v : Version @10ms", "provisioned externally"),
        ] {
            let checked = check_ridl("app", &format!("{PRELUDE}interface I {{\n  {member}\n}}\n"));
            assert_eq!(codes(&checked), vec!["RIDL-106"], "{kind}");
            let message = &checked.diagnostics[0].message;
            assert!(
                message.starts_with(&format!("a timing annotation is not valid on `{kind}`")),
                "{kind}: {message}",
            );
            assert!(
                message.contains(because),
                "{kind}: the message must say why this kind carries no timing: {message}",
            );
            assert!(
                message.contains("`signal` and `event`"),
                "{kind}: the message must name the kinds that do carry timing: {message}",
            );
        }
    }

    #[test]
    fn form_102_init_on_event_and_final() {
        let on_event = check_ridl(
            "app",
            &format!("{PRELUDE}interface I {{\n  event e : Speed = 3.0\n}}\n"),
        );
        // FORM-102 for the init; the untimed event then draws RIDL-100
        // (default applied, E2 task 9).
        assert_eq!(codes(&on_event), vec!["FORM-102", "RIDL-100"]);
        assert_eq!(
            on_event.diagnostics[0].message,
            "init value not valid on event",
        );

        let on_final = check_ridl(
            "app",
            &format!("{PRELUDE}interface I {{\n  final v : Version = \"1.0.0\"\n}}\n"),
        );
        assert_eq!(codes(&on_final), vec!["FORM-102"]);
        assert_eq!(
            on_final.diagnostics[0].message,
            "init value not valid on final",
        );
    }

    #[test]
    fn form_102_payload_and_param_narrowing() {
        // The grammar over-approximates payloads and params as FieldType
        // (task 3); the checker narrows to the reference Appendix C shapes.
        let tuple_signal = check_ridl(
            "app",
            &format!("{PRELUDE}interface I {{\n  signal s : (a: Speed, b: Speed) @10ms\n}}\n"),
        );
        assert_eq!(codes(&tuple_signal), vec!["FORM-102"]);
        assert_eq!(
            tuple_signal.diagnostics[0].message,
            "signal payload must be a named type",
        );

        let primitive_event = check_ridl(
            "app",
            &format!("{PRELUDE}interface I {{\n  event e : integer [0..5]\n}}\n"),
        );
        // FORM-102 for the payload; the untimed event then draws RIDL-100
        // (default applied, E2 task 9).
        assert_eq!(codes(&primitive_event), vec!["FORM-102", "RIDL-100"]);
        assert_eq!(
            primitive_event.diagnostics[0].message,
            "event payload must be a named type",
        );

        let map_final = check_ridl(
            "app",
            &format!("{PRELUDE}interface I {{\n  final f : [Version: Speed; 0..3]\n}}\n"),
        );
        assert_eq!(codes(&map_final), vec!["FORM-102"]);
        assert_eq!(
            map_final.diagnostics[0].message,
            "final payload must be a named type or an array",
        );

        let stream_final = check_ridl(
            "app",
            &format!("{PRELUDE}interface I {{\n  final f : <Speed>\n}}\n"),
        );
        assert_eq!(codes(&stream_final), vec!["FORM-102"]);
        assert_eq!(
            stream_final.diagnostics[0].message,
            "stream `<T>` not valid on final",
        );

        let tuple_param = check_ridl(
            "app",
            &format!("{PRELUDE}interface I {{\n  command c(p: (a: Speed))\n}}\n"),
        );
        assert_eq!(codes(&tuple_param), vec!["FORM-102"]);
        assert_eq!(
            tuple_param.diagnostics[0].message,
            "command parameter must be a named type or a stream",
        );

        // The Appendix C `final_type` admits arrays (`[Label; 0..32]`).
        let array_final = check_ridl(
            "app",
            &format!("{PRELUDE}interface I {{\n  final caps : [Label; 0..32]\n}}\n"),
        );
        assert!(
            codes(&array_final).is_empty(),
            "got: {:?}",
            array_final.diagnostics
        );
    }

    #[test]
    fn form_106_107_108_attribute_allow_list() {
        // gf §4.3: in E2 only `require`/`ensure` predicates are consumable —
        // a known key on the wrong kind is FORM-107, an unknown key FORM-106,
        // a duplicate key FORM-108.
        let known = check_ridl(
            "app",
            &format!("{PRELUDE}interface I {{\n  command c() [ persist ]\n}}\n"),
        );
        assert_eq!(codes(&known), vec!["FORM-107"]);
        assert!(
            known.diagnostics[0].message.contains("persist"),
            "got: {}",
            known.diagnostics[0].message,
        );

        let unknown = check_ridl(
            "app",
            &format!("{PRELUDE}interface I {{\n  command c() [ frobnicate ]\n}}\n"),
        );
        assert_eq!(codes(&unknown), vec!["FORM-106"]);

        let duplicate = check_ridl(
            "app",
            &format!("{PRELUDE}interface I {{\n  command c() [ persist, persist ]\n}}\n"),
        );
        assert_eq!(codes(&duplicate), vec!["FORM-107", "FORM-107", "FORM-108"]);

        let assignment = check_ridl(
            "app",
            &format!("{PRELUDE}interface I {{\n  signal s : Speed @10ms [ labels = (A) ]\n}}\n"),
        );
        assert_eq!(codes(&assignment), vec!["FORM-107"]);
    }

    #[test]
    fn typl_301_stream_on_a_struct_field_in_a_ridl_file() {
        // The ridl-profile parser accepts `<T>` in every type position and
        // defers the struct-field/collection rejection to the checker (task 3
        // hand-off; ridl §12.3 — typl TYPL-301 territory).
        let checked = check_ridl("app", &format!("{PRELUDE}struct S {{\n  x: <Speed>\n}}\n"));
        assert_eq!(codes(&checked), vec!["TYPL-301"]);
    }

    #[test]
    fn error_modifier_on_interface_is_rejected() {
        let checked = check_ridl("app", "package app\nerror interface I { }\n");
        assert_eq!(codes(&checked), vec!["TYPL-212"]);
        assert!(
            checked.diagnostics[0].message.contains("interface"),
            "got: {}",
            checked.diagnostics[0].message,
        );

        let internal = check_ridl("app", "package app\ninternal interface I { }\n");
        assert!(
            codes(&internal).is_empty(),
            "got: {:?}",
            internal.diagnostics
        );
    }

    #[test]
    fn interface_in_type_position_is_rejected() {
        let checked = check_ridl("app", "package app\ninterface I { }\nstruct S { f: I }\n");
        assert_eq!(
            messages(&checked),
            vec!["expected a type, but `I` names an interface"],
        );
    }

    #[test]
    fn payload_resolves_through_an_import_and_unknown_keeps_the_e1_message() {
        let mut db = RidlDatabase::default();
        let std = std_package(&mut db);
        let veh = package(
            &db,
            "veh.common",
            "package veh.common\ntype Speed: km/h [0.0..300.0 step 0.5]\n",
        );
        let app = ridl_package(
            &db,
            "app",
            "package app\nimport veh.common.Speed\ninterface I {\n  signal s : Speed @10ms\n}\n",
        );
        let ws = Workspace::new(&db, vec![app, veh], BTreeMap::new());
        let checked = check_package(&db, ws, app, std);
        assert!(
            checked.diagnostics.is_empty(),
            "the imported payload resolves, got: {:?}",
            checked.diagnostics,
        );

        let broken = check_ridl(
            "app",
            "package app\ninterface I {\n  signal s : NoSuchType @10ms\n}\n",
        );
        assert_eq!(messages(&broken), vec!["unknown type name `NoSuchType`"]);
    }

    // The ridl §11 ordinal assignment over the Appendix A interface — 1-based,
    // declaration order, one sequence across all kinds, the reserved tombstone
    // counted at #6 — is asserted on the lowered IR by
    // `appendix_a_interactions_carry_the_worked_ordinals` and
    // `appendix_a_tombstone_stores_its_ordinal_twice_and_they_agree` below.

    // --- the E2.1c lowering to IR v2 --------------------------------------

    /// The veh.common vocabulary the Appendix A contract package imports.
    const APPENDIX_A_COMMON: &str = "\
package veh.common

/// Vehicle speed over ground
type Speed       : km/h [0.0..MAX_SPEED step 0.5]

/// Coolant / ambient temperature
type Temperature : Cel  [-40.0..125.0 step 0.1]

const MAX_SPEED : Speed = 250.0

enum GearPosition {
  PARK    = 0
  DRIVE   = 1
  REVERSE = 2
  NEUTRAL = 3
}

enum Warning {
  LOW_FUEL     = 0
  CHECK_ENGINE = 1
  DOOR_OPEN    = 2
  SEATBELT     = 3
}

enumset WarningFlags : Warning
";

    /// The ridl reference Appendix A package — the local vocabulary plus the
    /// full `VehicleStatus` contract, member for member.
    const APPENDIX_A_PACKAGE: &str = "\
package veh.cluster

import veh.common.Speed
import veh.common.Temperature
import veh.common.MAX_SPEED
import veh.common.GearPosition
import veh.common.WarningFlags

// --- vocabulary local to this contract ---

struct DoorPayload {
  sensorId : integer [0..15]
  isOpen   : boolean
}

struct DiagFilter {
  severity : integer [0..5]
  category : Label?
}

struct FaultEvent {
  code      : integer [0..65535]
  message   : Message
  timestamp : Timestamp
}

error enum DiagError {
  FILTER_INVALID   = 0
  STORAGE_BUSY     = 1
  ACCESS_DENIED    = 2
}

struct FaultPage {
  faults : [FaultEvent; 0..64]
}

union FaultPageResult {
  page : FaultPage
  err  : DiagError
}

// --- the contract ---

/**
 * Main vehicle status interface.
 * @labels SIL_B, CAL_2, PRIVATE
 */
interface VehicleStatus {

  /// Current vehicle speed
  signal currentSpeed : Speed @10ms

  /// Engine temperature
  signal engineTemp : Temperature @[20ms..100ms]

  /// Active warnings
  signal warnings : WarningFlags @[50ms..1s]

  /// Raised on every door state change
  event doorOpened : DoorPayload @[50ms..500ms]

  /// Request a gear change
  command setGear(position: GearPosition) [
    require position != GearPosition.PARK || currentSpeed == 0.0
  ]

  reserved resetCounters

  /// Sliding-window average
  query getAverageSpeed(window: Duration): Speed [
    require window > 0ms
    ensure  result >= 0.0
  ]

  /// Fault history as a finite stream
  query streamFaults(filter: DiagFilter): <FaultEvent>

  /// Paged fault snapshot
  query getFaultPage(filter: DiagFilter): FaultPageResult

  final softwareVersion : Version
  final capabilities    : [Label; 0..32]
}
";

    /// Checks the two-package Appendix A workspace and returns the checked
    /// `veh.cluster` package.
    fn check_appendix_a() -> CheckedPackage {
        let mut db = RidlDatabase::default();
        let std = std_package(&mut db);
        let common = package(&db, "veh.common", APPENDIX_A_COMMON);
        let cluster = ridl_package(&db, "veh.cluster", APPENDIX_A_PACKAGE);
        let ws = Workspace::new(&db, vec![cluster, common], BTreeMap::new());
        check_package(&db, ws, cluster, std)
    }

    /// The interaction `Decl` named `name` in the first lowered interface.
    fn interaction<'a>(checked: &'a CheckedPackage, name: &str) -> &'a v2::Decl {
        checked
            .ir
            .interfaces
            .first()
            .unwrap_or_else(|| panic!("no interface lowered"))
            .interactions
            .iter()
            .find(|decl| decl.name == name)
            .unwrap_or_else(|| panic!("no interaction `{name}`"))
    }

    /// The `QueryDef` of the interaction named `name`.
    fn query_def<'a>(checked: &'a CheckedPackage, name: &str) -> &'a v2::QueryDef {
        let Some(v2::decl::Kind::QueryDef(query)) = &interaction(checked, name).kind else {
            panic!("`{name}` is not a query");
        };
        query
    }

    /// The `SignalDef` of the interaction named `name`.
    fn signal_def<'a>(checked: &'a CheckedPackage, name: &str) -> &'a v2::SignalDef {
        let Some(v2::decl::Kind::SignalDef(signal)) = &interaction(checked, name).kind else {
            panic!("`{name}` is not a signal");
        };
        signal
    }

    /// The `EventDef` of the interaction named `name`.
    fn event_def<'a>(checked: &'a CheckedPackage, name: &str) -> &'a v2::EventDef {
        let Some(v2::decl::Kind::EventDef(event)) = &interaction(checked, name).kind else {
            panic!("`{name}` is not an event");
        };
        event
    }

    #[test]
    fn appendix_a_lowers_clean_and_its_ir_v2_json_is_the_golden() {
        let checked = check_appendix_a();
        assert!(
            !checked
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.severity == Severity::Error),
            "Appendix A must lower without errors, got: {:?}",
            checked.diagnostics,
        );
        // The one advisory the reference's own worked example earns: its
        // `query getFaultPage(…): FaultPageResult` is the named-result-union
        // spelling, and the E2.10a lint (RIDL-308) steers return position to
        // the inline `FaultPage | DiagError` (general form §6.1). Appendix A
        // is kept verbatim rather than rewritten — the gf §7 erratum that
        // restates it is a documentation task, not this one.
        assert_eq!(
            codes(&checked),
            vec!["RIDL-308"],
            "{:?}",
            checked.diagnostics
        );
        assert_eq!(checked.ir.name, "veh.cluster");
        assert_eq!(checked.ir.interfaces.len(), 1);
        let interface = &checked.ir.interfaces[0];
        assert_eq!(interface.name, "VehicleStatus");
        assert_eq!(interface.visibility, v2::Visibility::Public as i32);
        assert_eq!(interface.doc, "Main vehicle status interface.");
        assert_eq!(interface.labels, ["SIL_B", "CAL_2", "PRIVATE"]);
        insta::assert_snapshot!("appendix_a_ir", v2::to_json_pretty(&checked.ir));
    }

    #[test]
    fn appendix_a_interactions_carry_the_worked_ordinals() {
        // ridl §11 via the T5 assignment: 1-based, declaration order, one
        // sequence across all kinds, the tombstone counted at #6 (its Decl
        // name stays empty — the retired name lives in `Reserved.name`).
        let checked = check_appendix_a();
        let walk: Vec<(&str, u32)> = checked.ir.interfaces[0]
            .interactions
            .iter()
            .map(|decl| (decl.name.as_str(), decl.ordinal))
            .collect();
        assert_eq!(
            walk,
            [
                ("currentSpeed", 1),
                ("engineTemp", 2),
                ("warnings", 3),
                ("doorOpened", 4),
                ("setGear", 5),
                ("", 6),
                ("getAverageSpeed", 7),
                ("streamFaults", 8),
                ("getFaultPage", 9),
                ("softwareVersion", 10),
                ("capabilities", 11),
            ],
        );
    }

    #[test]
    fn appendix_a_tombstone_stores_its_ordinal_twice_and_they_agree() {
        // The T1 review contract note: the tombstone's ordinal is stored on
        // the `Decl` envelope AND inside `Reserved`; the schema cannot
        // enforce the agreement, so the lowering must.
        let checked = check_appendix_a();
        let tombstone = &checked.ir.interfaces[0].interactions[5];
        assert_eq!(tombstone.ordinal, 6);
        let Some(v2::decl::Kind::ReservedSlot(reserved)) = &tombstone.kind else {
            panic!("#6 is not a reserved tombstone: {:?}", tombstone.kind);
        };
        assert_eq!(
            reserved.ordinal, tombstone.ordinal,
            "Decl.ordinal and Reserved.ordinal must agree",
        );
        assert_eq!(reserved.name.as_deref(), Some("resetCounters"));
    }

    #[test]
    fn appendix_a_stream_return_lowers_to_field_type_stream() {
        let checked = check_appendix_a();
        let query = query_def(&checked, "streamFaults");
        let Some(v2::field_type::Kind::Named(param)) =
            &query.params[0].r#type.as_ref().unwrap().kind
        else {
            panic!("the filter parameter is not a named type");
        };
        assert_eq!(param, "DiagFilter", "same-package references stay bare");
        let Some(v2::return_type::Kind::Value(value)) = &query.return_type.as_ref().unwrap().kind
        else {
            panic!("streamFaults does not return a plain value");
        };
        let Some(v2::field_type::Kind::Stream(stream)) = &value.kind else {
            panic!("streamFaults does not return a stream");
        };
        assert_eq!(
            stream.element,
            Some(v2::stream_type::Element::Named("FaultEvent".to_string())),
        );
    }

    #[test]
    fn appendix_a_named_result_union_returns_as_a_named_value() {
        // Appendix A's getFaultPage returns the NAMED union `FaultPageResult`
        // — a named `ReturnType.value`, never a synthesized fallible.
        let checked = check_appendix_a();
        let query = query_def(&checked, "getFaultPage");
        let Some(v2::return_type::Kind::Value(value)) = &query.return_type.as_ref().unwrap().kind
        else {
            panic!("getFaultPage does not return a plain value");
        };
        assert_eq!(
            value.kind,
            Some(v2::field_type::Kind::Named("FaultPageResult".to_string())),
        );
    }

    #[test]
    fn appendix_a_payloads_and_finals_lower_canonical_references() {
        let checked = check_appendix_a();
        // Imported payloads are fully qualified `pkg.Name` — never an alias,
        // never the bare imported spelling.
        assert_eq!(
            signal_def(&checked, "currentSpeed").payload,
            "veh.common.Speed"
        );
        assert_eq!(
            signal_def(&checked, "warnings").payload,
            "veh.common.WarningFlags"
        );
        // `final` payloads: an implicit `ridl.std` name is cross-package.
        let Some(v2::decl::Kind::FinalDef(version)) =
            &interaction(&checked, "softwareVersion").kind
        else {
            panic!("softwareVersion is not a final");
        };
        assert_eq!(
            version.payload.as_ref().unwrap().kind,
            Some(v2::field_type::Kind::Named("ridl.std.Version".to_string())),
        );
        let Some(v2::decl::Kind::FinalDef(caps)) = &interaction(&checked, "capabilities").kind
        else {
            panic!("capabilities is not a final");
        };
        let Some(v2::field_type::Kind::Array(array)) = &caps.payload.as_ref().unwrap().kind else {
            panic!("capabilities is not an array");
        };
        assert_eq!((array.min, array.max), (0, 32));
        assert_eq!(
            array.element.as_ref().unwrap().kind,
            Some(v2::field_type::Kind::Named("ridl.std.Label".to_string())),
        );
    }

    #[test]
    fn appendix_a_contracts_lower_kind_and_source_text() {
        // Task 11 lowers `Contract.kind` and the canonical source text; the
        // observer reference fields are task 12.
        let checked = check_appendix_a();
        let Some(v2::decl::Kind::CommandDef(set_gear)) = &interaction(&checked, "setGear").kind
        else {
            panic!("setGear is not a command");
        };
        assert_eq!(set_gear.contracts.len(), 1);
        assert_eq!(set_gear.contracts[0].kind, v2::ContractKind::Require as i32);
        assert_eq!(
            set_gear.contracts[0].source,
            "position != GearPosition.PARK || currentSpeed == 0.0",
        );
        let average = query_def(&checked, "getAverageSpeed");
        let kinds_and_sources: Vec<(i32, &str)> = average
            .contracts
            .iter()
            .map(|contract| (contract.kind, contract.source.as_str()))
            .collect();
        assert_eq!(
            kinds_and_sources,
            [
                (v2::ContractKind::Require as i32, "window > 0ms"),
                (v2::ContractKind::Ensure as i32, "result >= 0.0"),
            ],
        );
    }

    #[test]
    fn appendix_a_contracts_lower_observer_stubs() {
        // Task 12: every clause of the Appendix A interface is an addressable
        // observer stub — the reads it resolves plus the positional id E5/E7
        // observer tooling addresses it by. A signal read is canonical
        // `Interface.signalName`; a parameter read is the bare parameter name.
        let checked = check_appendix_a();
        assert_eq!(
            observer_stubs(&checked, "setGear"),
            [(
                "VehicleStatus.setGear.require[0]",
                vec!["VehicleStatus.currentSpeed"],
                vec!["position"],
                false,
            )],
        );
        assert_eq!(
            observer_stubs(&checked, "getAverageSpeed"),
            [
                (
                    "VehicleStatus.getAverageSpeed.require[0]",
                    vec![],
                    vec!["window"],
                    false,
                ),
                (
                    "VehicleStatus.getAverageSpeed.ensure[0]",
                    vec![],
                    vec![],
                    true,
                ),
            ],
        );
    }

    #[test]
    fn appendix_a_timing_resolves_concrete_bounds() {
        // Task 9 resolves every `@` annotation to exact microsecond bounds
        // (ADR-0008 decision 12): strict periodic stores the period in both
        // bounds; a range keeps each explicit bound.
        let checked = check_appendix_a();
        assert_eq!(
            signal_def(&checked, "currentSpeed").timing,
            Some(v2::Timing {
                mode: v2::TimingMode::StrictPeriodic as i32,
                min_us: Some("10000".to_string()),
                max_us: Some("10000".to_string()),
                default_applied: false,
            }),
            "@10ms lowers as a strict period with both bounds 10000us",
        );
        assert_eq!(
            signal_def(&checked, "engineTemp").timing,
            Some(v2::Timing {
                mode: v2::TimingMode::Range as i32,
                min_us: Some("20000".to_string()),
                max_us: Some("100000".to_string()),
                default_applied: false,
            }),
            "@[20ms..100ms] lowers as a range",
        );
        assert_eq!(
            event_def(&checked, "doorOpened").timing,
            Some(v2::Timing {
                mode: v2::TimingMode::Range as i32,
                min_us: Some("50000".to_string()),
                max_us: Some("500000".to_string()),
                default_applied: false,
            }),
            "@[50ms..500ms] lowers as a range on an event",
        );
    }

    #[test]
    fn ridl_100_untimed_signal_applies_the_default_with_the_flag() {
        // An untimed signal resolves the built-in default `[100ms..1000ms]`
        // with `default_applied` set, and draws a RIDL-100 warning naming the
        // applied bounds (ridl §9.1).
        let checked = check_ridl(
            "app",
            &format!("{PRELUDE}interface I {{\n  signal s : Speed\n}}\n"),
        );
        assert_eq!(codes(&checked), vec!["RIDL-100"]);
        assert_eq!(checked.diagnostics[0].severity, Severity::Warning);
        // The IR carries microseconds; the message does not. It names the
        // applied default in the units a `[defaults].timing` is written in.
        assert!(
            checked.diagnostics[0].message.contains("`@[100ms..1s]`"),
            "RIDL-100 must name the applied bounds as durations, got {:?}",
            checked.diagnostics[0].message,
        );
        assert_eq!(
            signal_def(&checked, "s").timing,
            Some(v2::Timing {
                mode: v2::TimingMode::Range as i32,
                min_us: Some("100000".to_string()),
                max_us: Some("1000000".to_string()),
                default_applied: true,
            }),
        );
    }

    #[test]
    fn fractional_durations_report_and_never_unset_a_written_bound_in_ir() {
        // The T9 review regression, asserted on the lowered IR: the lexer
        // merges a FloatNumber with a time atom, so a fractional duration
        // reaches the checker. Each case must draw a diagnostic AND keep every
        // bound the source wrote — a dropped min/max would be an invisible
        // contract change for `ridl diff` (ADR-0008 d12).
        let strict = check_ridl(
            "app",
            &format!("{PRELUDE}interface I {{\n  signal s : Speed @1.5ms\n}}\n"),
        );
        assert_eq!(codes(&strict), vec!["FORM-102"]);
        assert_eq!(
            signal_def(&strict, "s").timing,
            Some(v2::Timing {
                mode: v2::TimingMode::StrictPeriodic as i32,
                min_us: Some("1500".to_string()),
                max_us: Some("1500".to_string()),
                default_applied: false,
            }),
        );

        let low = check_ridl(
            "app",
            &format!("{PRELUDE}interface I {{\n  signal s : Speed @[1.5ms..100ms]\n}}\n"),
        );
        assert_eq!(codes(&low), vec!["FORM-102"]);
        assert_eq!(
            signal_def(&low, "s").timing,
            Some(v2::Timing {
                mode: v2::TimingMode::Range as i32,
                min_us: Some("1500".to_string()),
                max_us: Some("100000".to_string()),
                default_applied: false,
            }),
            "the rate floor must survive",
        );

        let high = check_ridl(
            "app",
            &format!("{PRELUDE}interface I {{\n  signal s : Speed @[20ms..2.5s]\n}}\n"),
        );
        assert_eq!(codes(&high), vec!["FORM-102"]);
        assert_eq!(
            signal_def(&high, "s").timing,
            Some(v2::Timing {
                mode: v2::TimingMode::Range as i32,
                min_us: Some("20000".to_string()),
                max_us: Some("2500000".to_string()),
                default_applied: false,
            }),
            "the staleness bound must survive",
        );

        // `0.0ms` is both an illegal form (FORM-102) and a zero value
        // (RIDL-102) — it must no longer escape the zero rule.
        let zero = check_ridl(
            "app",
            &format!("{PRELUDE}interface I {{\n  signal s : Speed @0.0ms\n}}\n"),
        );
        assert!(codes(&zero).contains(&"FORM-102"), "got {:?}", codes(&zero));
        assert!(codes(&zero).contains(&"RIDL-102"), "got {:?}", codes(&zero));
        assert_eq!(
            signal_def(&zero, "s").timing,
            Some(v2::Timing {
                mode: v2::TimingMode::StrictPeriodic as i32,
                min_us: Some("0".to_string()),
                max_us: Some("0".to_string()),
                default_applied: false,
            }),
        );
    }

    #[test]
    fn ridl_100_anchors_on_each_untimed_interaction() {
        // The T9 review fix: the default-applied warning points at the
        // interaction that received the default, so N untimed interactions
        // yield N navigable warnings rather than N carets on byte 0.
        let text =
            format!("{PRELUDE}interface I {{\n  signal alpha : Speed\n  event beta : Speed\n}}\n");
        let checked = check_ridl("app", &text);
        assert_eq!(codes(&checked), vec!["RIDL-100", "RIDL-100"]);

        let spans: Vec<&str> = checked
            .diagnostics
            .iter()
            .map(|diagnostic| {
                let range = diagnostic.primary.range;
                &text[usize::from(range.start())..usize::from(range.end())]
            })
            .collect();
        assert_eq!(
            spans,
            vec!["alpha", "beta"],
            "each RIDL-100 anchors on its own interaction name",
        );
    }

    #[test]
    fn a_configured_package_default_applies_to_untimed_signals() {
        // The package `[defaults].timing` resolves the bounds of an untimed
        // signal (ridl §9.1) — the raw string parses in the checker.
        let mut db = RidlDatabase::default();
        let std = std_package(&mut db);
        let pkg = ridl_package_with_default(
            &db,
            "app",
            &format!("{PRELUDE}interface I {{\n  signal s : Speed\n}}\n"),
            "[50ms..2s]",
        );
        let ws = Workspace::new(&db, vec![pkg], BTreeMap::new());
        let checked = check_package(&db, ws, pkg, std);
        assert_eq!(codes(&checked), vec!["RIDL-100"]);
        assert_eq!(
            signal_def(&checked, "s").timing,
            Some(v2::Timing {
                mode: v2::TimingMode::Range as i32,
                min_us: Some("50000".to_string()),
                max_us: Some("2000000".to_string()),
                default_applied: true,
            }),
            "the configured `[50ms..2s]` resolves to 50000us..2000000us",
        );
    }

    #[test]
    fn mani_009_on_a_malformed_default_timing_string() {
        // A malformed `[defaults].timing` is MANI-009, spanning the package's
        // first file; the built-in default is the fallback so signals still
        // resolve (ridl §9.1, ADR-0008 decision 13).
        let mut db = RidlDatabase::default();
        let std = std_package(&mut db);
        let pkg = ridl_package_with_default(
            &db,
            "app",
            &format!("{PRELUDE}interface I {{\n  signal s : Speed @10ms\n}}\n"),
            "fast",
        );
        let ws = Workspace::new(&db, vec![pkg], BTreeMap::new());
        let checked = check_package(&db, ws, pkg, std);
        assert_eq!(codes(&checked), vec!["MANI-009"]);
        assert_eq!(checked.diagnostics[0].severity, Severity::Error);
        assert!(
            checked.diagnostics[0].message.contains("[defaults].timing"),
            "MANI-009 must name the manifest key, got {:?}",
            checked.diagnostics[0].message,
        );
        // The strict-periodic signal still lowers with concrete bounds.
        assert_eq!(
            signal_def(&checked, "s").timing.as_ref().unwrap().min_us,
            Some("10000".to_string()),
        );
    }

    #[test]
    fn command_carries_no_timing_in_ir() {
        // A command has no `Timing` field at all — timing is absent from the IR
        // for command/query/final (ridl §9).
        let checked = check_ridl(
            "app",
            &format!("{PRELUDE}interface I {{\n  command c(p: Speed)\n}}\n"),
        );
        assert!(codes(&checked).is_empty(), "got: {:?}", checked.diagnostics);
        let v2::decl::Kind::CommandDef(_) = interaction(&checked, "c").kind.as_ref().unwrap()
        else {
            panic!("expected a command");
        };
        // `CommandDef` has no timing field — nothing to assert absent beyond
        // the type shape; the absence is structural.
    }

    #[test]
    fn inline_fallible_return_lowers_as_return_type_fallible() {
        // The corpus variant of Appendix A's getFaultPage: the inline
        // `FaultPage | DiagError` form (gf §6.1) lowers as
        // `ReturnType.fallible` with both arms canonical.
        let checked = check_ridl(
            "app",
            "package app\nstruct FaultPage {\n  count : integer [0..64]\n}\nerror enum DiagError {\n  STORAGE_BUSY = 0\n}\ninterface I {\n  query getFaultPage(): FaultPage | DiagError\n}\n",
        );
        assert!(codes(&checked).is_empty(), "got: {:?}", checked.diagnostics);
        let query = query_def(&checked, "getFaultPage");
        let Some(v2::return_type::Kind::Fallible(fallible)) =
            &query.return_type.as_ref().unwrap().kind
        else {
            panic!("the inline T | E return did not lower as fallible");
        };
        assert_eq!(fallible.ok, "FaultPage");
        assert_eq!(fallible.err, "DiagError");
    }

    // --- E2.3 errors-as-data: inline `T | E` semantics (task 10) ----------

    /// A clean fallible-return vocabulary: a success struct, an error enum, a
    /// param type, and a non-error scalar for the arm-rule tests.
    const FALLIBLE_VOCAB: &str = "package app\nstruct CalReport {\n  count : integer [0..64]\n}\nerror enum CalError {\n  SENSOR_UNAVAILABLE = 0\n}\ntype Axle: integer [0..3]\ntype Speed: km/h [0.0..300.0 step 0.5]\n";

    #[test]
    fn fallible_query_checks_clean_and_lowers_canonical_arms() {
        // gf §6.1: `query calibrate(axle: Axle): CalReport | CalError` — a
        // non-error left arm, exactly one error right arm — checks clean and
        // lowers as `ReturnType.fallible` with both arms canonical.
        let checked = check_ridl(
            "app",
            &format!(
                "{FALLIBLE_VOCAB}interface I {{\n  query calibrate(axle: Axle): CalReport | CalError\n}}\n"
            ),
        );
        assert!(codes(&checked).is_empty(), "got: {:?}", checked.diagnostics);
        let query = query_def(&checked, "calibrate");
        let Some(v2::return_type::Kind::Fallible(fallible)) =
            &query.return_type.as_ref().unwrap().kind
        else {
            panic!("calibrate did not lower as fallible");
        };
        assert_eq!(fallible.ok, "CalReport");
        assert_eq!(fallible.err, "CalError");
        // ADR-0008 decision 4: the transport identity is not stored — a
        // consumer derives it from the interface, the ordinal, and the arms.
        let decl = interaction(&checked, "calibrate");
        assert_eq!(
            v2::fallible_transport_identity("I", decl.ordinal, fallible),
            "I#1:CalReport|CalError",
        );
    }

    #[test]
    fn fallible_arm_err_lowers_cross_package_canonical() {
        // The IR JSON shows `"fallible": { "ok": "CalReport", "err":
        // "veh.common.CalError" }` — a same-package success arm bare, a
        // cross-package error arm fully qualified.
        let mut db = RidlDatabase::default();
        let std = std_package(&mut db);
        let common = package(
            &db,
            "veh.common",
            "package veh.common\nerror enum CalError {\n  SENSOR_UNAVAILABLE = 0\n}\n",
        );
        let app = ridl_package(
            &db,
            "app",
            "package app\nimport veh.common.CalError\nstruct CalReport {\n  count : integer [0..64]\n}\ntype Axle: integer [0..3]\ninterface I {\n  query calibrate(axle: Axle): CalReport | CalError\n}\n",
        );
        let ws = Workspace::new(&db, vec![app, common], BTreeMap::new());
        let checked = check_package(&db, ws, app, std);
        assert!(codes(&checked).is_empty(), "got: {:?}", checked.diagnostics);
        let query = query_def(&checked, "calibrate");
        let Some(v2::return_type::Kind::Fallible(fallible)) =
            &query.return_type.as_ref().unwrap().kind
        else {
            panic!("calibrate did not lower as fallible");
        };
        assert_eq!(fallible.ok, "CalReport");
        assert_eq!(fallible.err, "veh.common.CalError");
        let json = v2::to_json_pretty(&checked.ir);
        assert!(
            json.contains("\"ok\": \"CalReport\"")
                && json.contains("\"err\": \"veh.common.CalError\""),
            "IR JSON must show the canonical fallible arms; got:\n{json}",
        );
    }

    #[test]
    fn swapped_arms_success_arm_is_error_draws_ridl_303() {
        // `CalError | CalReport` — the error arm written first. Exactly one
        // RIDL-303 naming the arm-order mistake; the non-error right arm is not
        // separately reported.
        let checked = check_ridl(
            "app",
            &format!(
                "{FALLIBLE_VOCAB}interface I {{\n  query calibrate(axle: Axle): CalError | CalReport\n}}\n"
            ),
        );
        assert_eq!(codes(&checked), vec!["RIDL-303"]);
        assert!(
            checked.diagnostics[0].message.contains("success arm")
                && checked.diagnostics[0].message.contains("is an error type"),
            "got: {}",
            checked.diagnostics[0].message,
        );
    }

    #[test]
    fn right_arm_not_error_draws_ridl_303() {
        // `CalReport | Speed` — the right arm is a non-error type. RIDL-303
        // with the gf §6.1 wording.
        let checked = check_ridl(
            "app",
            &format!(
                "{FALLIBLE_VOCAB}interface I {{\n  query calibrate(axle: Axle): CalReport | Speed\n}}\n"
            ),
        );
        assert_eq!(codes(&checked), vec!["RIDL-303"]);
        assert!(
            checked.diagnostics[0]
                .message
                .contains("is not an error type"),
            "got: {}",
            checked.diagnostics[0].message,
        );
    }

    #[test]
    fn bare_error_type_return_draws_ridl_303() {
        // `query f(): CalError` — a bare error type has no success path.
        let checked = check_ridl(
            "app",
            &format!(
                "{FALLIBLE_VOCAB}interface I {{\n  query calibrate(axle: Axle): CalError\n}}\n"
            ),
        );
        assert_eq!(codes(&checked), vec!["RIDL-303"]);
        // It still lowers honestly as a named value (the shape is representable).
        let query = query_def(&checked, "calibrate");
        assert!(matches!(
            &query.return_type.as_ref().unwrap().kind,
            Some(v2::return_type::Kind::Value(_)),
        ));
    }

    #[test]
    fn both_arms_error_draws_ridl_303() {
        // `CalError | CalError` — the success (left) arm is an error type.
        let checked = check_ridl(
            "app",
            &format!(
                "{FALLIBLE_VOCAB}interface I {{\n  query calibrate(axle: Axle): CalError | CalError\n}}\n"
            ),
        );
        assert_eq!(codes(&checked), vec!["RIDL-303"]);
        assert!(
            checked.diagnostics[0].message.contains("success arm"),
            "got: {}",
            checked.diagnostics[0].message,
        );
    }

    #[test]
    fn error_typed_param_draws_ridl_304() {
        // `command c(e: CalError)` — an error-typed parameter (warning).
        let checked = check_ridl(
            "app",
            &format!("{FALLIBLE_VOCAB}interface I {{\n  command c(e: CalError)\n}}\n"),
        );
        assert_eq!(codes(&checked), vec!["RIDL-304"]);
        assert_eq!(checked.diagnostics[0].severity, Severity::Warning);
    }

    #[test]
    fn result_union_param_draws_ridl_304() {
        // A result-union parameter also flows failure toward a provider
        // (RIDL-304, warning) — recomputed from the union's arms.
        let checked = check_ridl(
            "app",
            &format!(
                "{FALLIBLE_VOCAB}union CalOutcome {{\n  ok : CalReport\n  err : CalError\n}}\ninterface I {{\n  query q(outcome: CalOutcome): CalReport | CalError\n}}\n"
            ),
        );
        assert_eq!(codes(&checked), vec!["RIDL-304"]);
        assert_eq!(checked.diagnostics[0].severity, Severity::Warning);
    }

    #[test]
    fn error_enum_stratum_2_category_draws_ridl_307() {
        // An `error` enum declaring a reserved Stratum-2 category name draws
        // RIDL-307 (warning) — checked wherever error enums lower, both
        // profiles. All four category names fire.
        for category in [
            "INVALID_VALUE",
            "PRECONDITION_FAILED",
            "CONTRACT_BROKEN",
            "UNKNOWN_INTERACTION",
        ] {
            let checked = check_ridl(
                "app",
                &format!("package app\nerror enum X {{\n  {category} = 0\n}}\n"),
            );
            assert_eq!(codes(&checked), vec!["RIDL-307"], "category `{category}`");
            assert_eq!(checked.diagnostics[0].severity, Severity::Warning);
        }
    }

    #[test]
    fn ordinary_error_enum_value_is_clean() {
        // A non-reserved value name in an error enum draws nothing.
        let checked = check_ridl(
            "app",
            "package app\nerror enum X {\n  SENSOR_UNAVAILABLE = 0\n}\n",
        );
        assert!(codes(&checked).is_empty(), "got: {:?}", checked.diagnostics);
    }

    #[test]
    fn named_result_union_return_stays_legal() {
        // gf §6.1: a named result union in return position is legal typl data
        // and lowers as `ReturnType.value`. It draws the canonical-form lint
        // (RIDL-308, task 19) — a warning steering to the inline spelling,
        // never an error, and the lowering below is unaffected by it.
        let checked = check_ridl(
            "app",
            &format!(
                "{FALLIBLE_VOCAB}union CalOutcome {{\n  ok : CalReport\n  err : CalError\n}}\ninterface I {{\n  query calibrate(axle: Axle): CalOutcome\n}}\n"
            ),
        );
        assert_eq!(
            codes(&checked),
            vec!["RIDL-308"],
            "got: {:?}",
            checked.diagnostics
        );
        let query = query_def(&checked, "calibrate");
        assert_eq!(
            query.return_type.as_ref().unwrap().kind,
            Some(v2::return_type::Kind::Value(v2::FieldType {
                optional: false,
                kind: Some(v2::field_type::Kind::Named("CalOutcome".to_string())),
            })),
        );
    }

    #[test]
    fn mixed_typl_and_ridl_files_lower_both_surfaces_into_one_package() {
        let mut db = RidlDatabase::default();
        let std = std_package(&mut db);
        let typl = InputFile::new(
            &db,
            "app/types.typl".to_string(),
            "package app\ntype Speed: km/h [0.0..300.0 step 0.5]\n".to_string(),
        );
        let ridl = InputFile::new(
            &db,
            "app/contract.ridl".to_string(),
            "package app\ninterface I {\n  signal s : Speed @10ms\n}\n".to_string(),
        );
        let pkg = Package::new(
            &db,
            "app".to_string(),
            vec![typl, ridl],
            PackageOrigin::WorkspaceMember,
            BTreeMap::new(),
            None,
        );
        let ws = Workspace::new(&db, vec![pkg], BTreeMap::new());
        let checked = check_package(&db, ws, pkg, std);
        assert!(
            checked.diagnostics.is_empty(),
            "got: {:?}",
            checked.diagnostics
        );
        assert_eq!(checked.ir.decls.len(), 1, "the typl surface lowers");
        assert_eq!(checked.ir.decls[0].name, "Speed");
        assert_eq!(checked.ir.decls[0].ordinal, 0);
        assert_eq!(checked.ir.interfaces.len(), 1, "the ridl surface lowers");
        assert_eq!(signal_def(&checked, "s").payload, "Speed");
    }

    #[test]
    fn signal_declared_init_lowers_on_the_real_path() {
        // The T1 fixture coverage gap closes here: the bare `= value`
        // channel-init override (ADR-0008 decision 2) reaches
        // `SignalDef.declared_init` from real source, in canonical decimal
        // text, and `init` resolves to it. A constant reference resolves
        // through the E1 const chain.
        let checked = check_ridl(
            "app",
            &format!(
                "{PRELUDE}const CRUISE = 120.0\ninterface I {{\n  signal a : Speed = 42.0 @10ms\n  signal b : Speed = CRUISE @10ms\n  signal c : Speed @10ms\n}}\n"
            ),
        );
        assert!(codes(&checked).is_empty(), "got: {:?}", checked.diagnostics);
        let a = signal_def(&checked, "a");
        assert_eq!(a.declared_init.as_deref(), Some("42"));
        assert_eq!(a.init.as_ref().unwrap().value.as_deref(), Some("42"));
        let b = signal_def(&checked, "b");
        assert_eq!(b.declared_init.as_deref(), Some("120"));
        assert_eq!(b.init.as_ref().unwrap().value.as_deref(), Some("120"));
        // No override: the payload type's own derived init rides along.
        let c = signal_def(&checked, "c");
        assert_eq!(c.declared_init, None);
        assert!(c.init.as_ref().unwrap().derivable);
    }

    #[test]
    fn interaction_doc_tags_lower_into_the_decl_envelope() {
        // The E1 doc-tag scanner applied to interactions: prose body in
        // `doc`, `@labels` in `labels`, `@deprecated` in `deprecated`.
        let checked = check_ridl(
            "app",
            &format!(
                "{PRELUDE}interface I {{\n  /// Current speed\n  /// @labels SAFETY(D), CAL_1\n  /// @deprecated \"use speedV2\"\n  signal speed : Speed @10ms\n}}\n"
            ),
        );
        assert!(codes(&checked).is_empty(), "got: {:?}", checked.diagnostics);
        let decl = interaction(&checked, "speed");
        assert_eq!(decl.doc, "Current speed");
        assert_eq!(decl.labels, ["SAFETY(D)", "CAL_1"]);
        assert_eq!(decl.deprecated.as_deref(), Some("use speedV2"));
        // Interactions carry no visibility and no error modifier.
        assert_eq!(decl.visibility, v2::Visibility::Unspecified as i32);
        assert!(!decl.is_error);
    }

    #[test]
    fn payload_lowers_the_canonical_reference_never_the_alias() {
        let mut db = RidlDatabase::default();
        let std = std_package(&mut db);
        let veh = package(
            &db,
            "veh.common",
            "package veh.common\ntype Speed: km/h [0.0..300.0 step 0.5]\n",
        );
        let app = ridl_package(
            &db,
            "app",
            "package app\nimport veh.common.Speed as Velocity\ninterface I {\n  signal s : Velocity @10ms\n}\n",
        );
        let ws = Workspace::new(&db, vec![app, veh], BTreeMap::new());
        let checked = check_package(&db, ws, app, std);
        assert!(
            checked.diagnostics.is_empty(),
            "got: {:?}",
            checked.diagnostics
        );
        assert_eq!(signal_def(&checked, "s").payload, "veh.common.Speed");
    }

    #[test]
    fn ridl_402_duplicate_lowers_first_wins_into_the_ir() {
        let checked = check_ridl(
            "app",
            &format!(
                "{PRELUDE}interface I {{\n  signal a : Speed @10ms\n  event a : Speed\n  query b(): Speed\n}}\n"
            ),
        );
        assert_eq!(codes(&checked), vec!["RIDL-402"]);
        let walk: Vec<(&str, u32, bool)> = checked.ir.interfaces[0]
            .interactions
            .iter()
            .map(|decl| {
                (
                    decl.name.as_str(),
                    decl.ordinal,
                    matches!(decl.kind, Some(v2::decl::Kind::SignalDef(_))),
                )
            })
            .collect();
        // The losing `event a` re-declaration is excluded, holds no ordinal
        // slot, and the surviving contract stays contiguous.
        assert_eq!(walk, [("a", 1, true), ("b", 2, false)]);
    }

    // --- nameless members (parse recovery) -------------------------------

    /// The interaction `(name, ordinal)` walk of the first interface.
    fn interface_walk(checked: &CheckedPackage) -> Vec<(&str, u32)> {
        checked.ir.interfaces[0]
            .interactions
            .iter()
            .map(|decl| (decl.name.as_str(), decl.ordinal))
            .collect()
    }

    /// The interaction `(name, ordinal)` walk of the first service's inline
    /// shape.
    fn inline_walk(checked: &CheckedPackage) -> Vec<(&str, u32)> {
        let Some(v2::service::Shape::Inline(inline)) = &checked.ir.services[0].shape else {
            panic!("the service must lower to an inline shape");
        };
        inline
            .interactions
            .iter()
            .map(|decl| (decl.name.as_str(), decl.ordinal))
            .collect()
    }

    /// A member the parser recovered with no name at all (FORM-101) does not
    /// lower: an empty `Decl.name` is not a name a backend can emit. It still
    /// consumes its ordinal slot, so every later interaction keeps the ordinal
    /// its author wrote — the property that makes error recovery and the
    /// inlay hints usable.
    #[test]
    fn form_101_nameless_interaction_does_not_lower_but_holds_its_ordinal() {
        let checked = check_ridl(
            "app",
            &format!(
                "{PRELUDE}interface I {{\n  signal : Speed @10ms\n  signal after : Speed @10ms\n}}\n"
            ),
        );
        assert_eq!(interface_walk(&checked), [("after", 2)]);
    }

    /// FORM-105 — a family reserved word (`view` belongs to uxdl, so it is
    /// reserved but not an active ridl keyword) used as an interaction name.
    /// The parser holds it in an `ErrorNode` rather than a `Name`, so the
    /// member reaches the lowering nameless, exactly as FORM-101 does.
    #[test]
    fn form_105_reserved_word_interaction_does_not_lower_but_holds_its_ordinal() {
        let checked = check_ridl(
            "app",
            &format!(
                "{PRELUDE}interface I {{\n  signal view : Speed @10ms\n  signal after : Speed @10ms\n}}\n"
            ),
        );
        assert_eq!(interface_walk(&checked), [("after", 2)]);
    }

    /// The nameless-member rule is the inline service shape's too — the two
    /// lowering loops must stay behaviorally identical.
    #[test]
    fn form_101_nameless_inline_interaction_does_not_lower_but_holds_its_ordinal() {
        let checked = check_ridl(
            "app",
            &format!(
                "{PRELUDE}service veh.a.b {{\n  signal : Speed @10ms\n  signal after : Speed @10ms\n}}\n"
            ),
        );
        assert_eq!(inline_walk(&checked), [("after", 2)]);
    }

    #[test]
    fn form_105_reserved_word_inline_interaction_does_not_lower_but_holds_its_ordinal() {
        let checked = check_ridl(
            "app",
            &format!(
                "{PRELUDE}service veh.a.b {{\n  signal view : Speed @10ms\n  signal after : Speed @10ms\n}}\n"
            ),
        );
        assert_eq!(inline_walk(&checked), [("after", 2)]);
    }

    /// No interaction ever lowers with an empty name. A tombstone's `Decl`
    /// name is empty by design (the retired name lives in `Reserved.name`),
    /// so the sweep excludes reserved slots.
    #[test]
    fn no_nameless_interaction_reaches_the_ir() {
        for body in [
            "interface I {\n  signal : Speed @10ms\n  signal after : Speed @10ms\n}\n",
            "interface I {\n  event : Speed\n  signal after : Speed @10ms\n}\n",
            "interface I {\n  command (g: Speed)\n  signal after : Speed @10ms\n}\n",
            "interface I {\n  query (): Speed\n  signal after : Speed @10ms\n}\n",
            "interface I {\n  final : Speed = 1.0\n  signal after : Speed @10ms\n}\n",
            "interface I {\n  signal view : Speed @10ms\n  signal after : Speed @10ms\n}\n",
            "service veh.a.b {\n  signal : Speed @10ms\n  signal after : Speed @10ms\n}\n",
            "service veh.a.b {\n  signal view : Speed @10ms\n  signal after : Speed @10ms\n}\n",
        ] {
            let checked = check_ridl("app", &format!("{PRELUDE}{body}"));
            let mut interactions: Vec<&v2::Decl> = checked
                .ir
                .interfaces
                .iter()
                .flat_map(|interface| interface.interactions.iter())
                .collect();
            for service in &checked.ir.services {
                if let Some(v2::service::Shape::Inline(inline)) = &service.shape {
                    interactions.extend(inline.interactions.iter());
                }
            }
            for decl in interactions {
                if matches!(decl.kind, Some(v2::decl::Kind::ReservedSlot(_))) {
                    continue;
                }
                assert!(
                    !decl.name.is_empty(),
                    "an empty-named interaction reached the IR from: {body}",
                );
            }
        }
    }

    /// A service the parser recovered without a name does not lower either.
    /// A service is published at its dotted global name, so an empty
    /// `Service.name` would publish at the empty address — the same defect
    /// class as an empty `Decl.name`, and reachable from both service forms.
    /// FORM-101 already reports it, so no second diagnostic is raised.
    #[test]
    fn form_101_nameless_service_does_not_lower() {
        for body in [
            "service {\n  signal a : Speed @10ms\n}\n",
            "interface I {\n  signal a : Speed @10ms\n}\nservice : I\n",
        ] {
            let checked = check_ridl("app", &format!("{PRELUDE}{body}"));
            assert!(
                checked.ir.services.is_empty(),
                "a nameless service reached the IR from: {body}",
            );
            // FORM-101 is the parser's, and it is merged in by
            // `ridlc::compile`; the checker adds nothing of its own.
            assert_eq!(codes(&checked), Vec::<&str>::new(), "from: {body}");
        }
    }

    /// The guard is on the empty name only — a named service still lowers,
    /// and a same-package duplicate still lowers once, first-wins.
    #[test]
    fn named_services_still_lower_first_wins() {
        let checked = check_ridl(
            "app",
            &format!(
                "{PRELUDE}service veh.a.b {{\n  signal a : Speed @10ms\n}}\nservice veh.a.b {{\n  signal b : Speed @10ms\n}}\n"
            ),
        );
        let names: Vec<&str> = checked
            .ir
            .services
            .iter()
            .map(|service| service.name.as_str())
            .collect();
        assert_eq!(names, ["veh.a.b"]);
    }

    // --- services (E2.13, ridl reference §14.5) --------------------------

    #[test]
    fn service_naming_an_interface_lowers_to_the_ir() {
        let checked = check_ridl(
            "app",
            &format!(
                "{PRELUDE}interface CruiseControl {{\n  signal engaged : Speed @10ms\n}}\nservice veh.adas.cruise : CruiseControl\n"
            ),
        );
        assert!(codes(&checked).is_empty(), "got: {:?}", checked.diagnostics);
        assert_eq!(checked.ir.services.len(), 1);
        let service = &checked.ir.services[0];
        assert_eq!(service.name, "veh.adas.cruise");
        assert_eq!(service.visibility, v2::Visibility::Public as i32);
        assert_eq!(
            service.shape,
            Some(v2::service::Shape::InterfaceRef(
                "CruiseControl".to_string()
            )),
        );
    }

    #[test]
    fn ridl_141_service_names_a_type_not_an_interface() {
        let checked = check_ridl("app", &format!("{PRELUDE}service veh.adas.speed : Speed\n"));
        assert!(
            codes(&checked).contains(&"RIDL-141"),
            "got: {:?}",
            checked.diagnostics,
        );
        // The service still lowers — a diagnostic does not suppress the shape.
        assert_eq!(checked.ir.services.len(), 1);
        assert_eq!(
            checked.ir.services[0].shape,
            Some(v2::service::Shape::InterfaceRef("Speed".to_string())),
        );
    }

    #[test]
    fn service_inline_shape_lowers_to_an_anonymous_interface() {
        let checked = check_ridl(
            "app",
            &format!(
                "{PRELUDE}service veh.hvac.cabin {{\n  signal temperature : Speed @10ms\n  command setTarget(t: Speed)\n}}\n"
            ),
        );
        assert!(codes(&checked).is_empty(), "got: {:?}", checked.diagnostics);
        assert_eq!(checked.ir.services.len(), 1);
        let Some(v2::service::Shape::Inline(inline)) = &checked.ir.services[0].shape else {
            panic!("inline service must lower to an inline shape");
        };
        // The inline interface is anonymous (ridl §14.5) with its own ordinal
        // sequence.
        assert_eq!(inline.name, "");
        assert_eq!(inline.visibility, v2::Visibility::Unspecified as i32);
        let walk: Vec<(&str, u32)> = inline
            .interactions
            .iter()
            .map(|decl| (decl.name.as_str(), decl.ordinal))
            .collect();
        assert_eq!(walk, [("temperature", 1), ("setTarget", 2)]);
    }

    #[test]
    fn service_inline_shape_runs_the_structural_pass() {
        let checked = check_ridl(
            "app",
            &format!(
                "{PRELUDE}service veh.hvac.cabin {{\n  signal temperature : Speed @10ms\n  signal temperature : Speed @10ms\n}}\n"
            ),
        );
        // The inline body runs the same duplicate-interaction check an
        // interface body does (RIDL-402, first-wins).
        assert_eq!(codes(&checked), vec!["RIDL-402"]);
        let Some(v2::service::Shape::Inline(inline)) = &checked.ir.services[0].shape else {
            panic!("inline service must lower to an inline shape");
        };
        assert_eq!(inline.interactions.len(), 1);
    }

    #[test]
    fn service_inline_shape_require_reads_its_sibling_signals() {
        // An inline shape IS an interface shape (ridl §14.5), so a `require`
        // on one of its interactions may read the shape's own signals exactly
        // as it may inside an `interface` body (ridl §13). Reading one is
        // clean, and it lowers as a canonical signal ref scoped to the
        // service's dotted global name.
        let checked = check_ridl(
            "app",
            &format!(
                "{PRELUDE}service veh.hvac.cabin {{\n  signal temperature : Speed @10ms\n  command setTarget(t: Speed) [ require temperature < t ]\n}}\n"
            ),
        );
        assert!(codes(&checked).is_empty(), "got: {:?}", checked.diagnostics);
        let Some(v2::service::Shape::Inline(inline)) = &checked.ir.services[0].shape else {
            panic!("inline service must lower to an inline shape");
        };
        let Some(v2::decl::Kind::CommandDef(set_target)) = &inline
            .interactions
            .iter()
            .find(|decl| decl.name == "setTarget")
            .expect("no `setTarget` interaction")
            .kind
        else {
            panic!("setTarget is not a command");
        };
        assert_eq!(set_target.contracts.len(), 1);
        assert_eq!(
            set_target.contracts[0].signal_refs,
            ["veh.hvac.cabin.temperature"],
        );
        assert_eq!(set_target.contracts[0].param_refs, ["t"]);
        assert_eq!(
            set_target.contracts[0].observer_id,
            "veh.hvac.cabin.setTarget.require[0]",
        );
    }

    // --- catalog/IR parity on the canonical interface reference ----------
    //
    // The catalog is the SSOT later tasks consume, so its `interface_ref` must
    // agree with the IR's on every clean program. Both cases below are clean —
    // no diagnostic on either side would reveal a divergence.

    /// A workspace-member package whose files are all `.ridl`.
    fn ridl_package_files(db: &RidlDatabase, name: &str, files: &[(&str, &str)]) -> Package {
        let inputs = files
            .iter()
            .map(|(file_name, text)| {
                InputFile::new(
                    db,
                    format!("{}/{file_name}", name.replace('.', "/")),
                    text.to_string(),
                )
            })
            .collect();
        Package::new(
            db,
            name.to_string(),
            inputs,
            PackageOrigin::WorkspaceMember,
            BTreeMap::new(),
            None,
        )
    }

    /// The named shape of the one service in a checked package.
    fn service_ref(checked: &CheckedPackage) -> &str {
        let Some(v2::service::Shape::InterfaceRef(reference)) = &checked.ir.services[0].shape
        else {
            panic!("expected a named service shape");
        };
        reference
    }

    const CRUISE_CONTROL: &str = "package veh.common\ntype Flag: boolean\ninterface CruiseControl {\n  signal engaged : Flag @10ms\n}\n";

    #[test]
    fn catalog_and_ir_agree_when_the_import_is_in_another_file() {
        let mut db = RidlDatabase::default();
        let std = std_package(&mut db);
        let common = ridl_package(&db, "veh.common", CRUISE_CONTROL);
        // Imports bind package-wide (ADR-0002 §2): the import sits in one file
        // and the service in another, so a file-scoped scan would miss it.
        let adas = ridl_package_files(
            &db,
            "veh.adas",
            &[
                (
                    "a.ridl",
                    "package veh.adas\nimport veh.common.CruiseControl\n",
                ),
                (
                    "b.ridl",
                    "package veh.adas\nservice veh.adas.cruise : CruiseControl\n",
                ),
            ],
        );
        let ws = Workspace::new(&db, vec![common, adas], BTreeMap::new());

        let checked = check_package(&db, ws, adas, std);
        let catalog = service_catalog(&db, ws, std);

        assert!(
            checked.diagnostics.is_empty(),
            "got: {:?}",
            checked.diagnostics
        );
        assert!(catalog.diagnostics.is_empty());
        assert_eq!(service_ref(&checked), "veh.common.CruiseControl");
        assert_eq!(
            catalog.entries["veh.adas.cruise"].interface_ref,
            service_ref(&checked),
            "the catalog and the IR must agree on the canonical reference",
        );
    }

    #[test]
    fn catalog_and_ir_agree_when_a_local_shadows_an_import() {
        let mut db = RidlDatabase::default();
        let std = std_package(&mut db);
        let common = ridl_package(&db, "veh.common", CRUISE_CONTROL);
        // A local declaration shadows the import of the same name, so the
        // reference resolves same-package and canonicalizes bare.
        let adas = ridl_package(
            &db,
            "veh.adas",
            "package veh.adas\nimport veh.common.CruiseControl\ntype Flag: boolean\ninterface CruiseControl {\n  signal engaged : Flag @10ms\n}\nservice veh.adas.cruise : CruiseControl\n",
        );
        let ws = Workspace::new(&db, vec![common, adas], BTreeMap::new());

        let checked = check_package(&db, ws, adas, std);
        let catalog = service_catalog(&db, ws, std);

        assert!(
            checked.diagnostics.is_empty(),
            "got: {:?}",
            checked.diagnostics
        );
        assert!(catalog.diagnostics.is_empty());
        assert_eq!(service_ref(&checked), "CruiseControl");
        assert_eq!(
            catalog.entries["veh.adas.cruise"].interface_ref,
            service_ref(&checked),
            "the catalog and the IR must agree on the canonical reference",
        );
    }

    #[test]
    fn service_name_segments_must_be_lowercase() {
        let checked = check_ridl(
            "app",
            &format!(
                "{PRELUDE}interface I {{\n  signal a : Speed @10ms\n}}\nservice veh.X.y : I\n"
            ),
        );
        let messages = messages(&checked);
        assert_eq!(messages.len(), 1, "got: {:?}", checked.diagnostics);
        assert!(
            messages[0].contains("`X` is not lowercase"),
            "got: {:?}",
            messages,
        );

        let good = check_ridl(
            "app",
            &format!(
                "{PRELUDE}interface I {{\n  signal a : Speed @10ms\n}}\nservice veh.x2.y : I\n"
            ),
        );
        assert!(codes(&good).is_empty(), "got: {:?}", good.diagnostics);
    }

    #[test]
    fn a_same_package_duplicate_service_lowers_once() {
        let checked = check_ridl(
            "app",
            &format!(
                "{PRELUDE}interface I {{\n  signal a : Speed @10ms\n}}\nservice veh.adas.cruise : I\nservice veh.adas.cruise : I\n"
            ),
        );
        // The workspace-wide RIDL-140 is `service_catalog`'s job; the IR must
        // not carry the name twice regardless.
        assert_eq!(checked.ir.services.len(), 1);
        assert_eq!(checked.ir.services[0].name, "veh.adas.cruise");
    }
}
