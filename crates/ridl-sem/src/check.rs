//! The package checker: lowers every declaration of a resolved package to IR
//! v1 and runs the typl composite and scalar checks (docs/ROADMAP.md epic
//! E1.7a; typl language reference §4–§12, §16).
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

use std::collections::HashSet;

use ridl_core::db::InputFile;
use ridl_core::diag::{DiagCode, Diagnostic, FileId, Severity, SourceMap, Span};
use ridl_core::package::{Package, Workspace, package_of};
use ridl_ir::v1;
use ridl_syntax::SyntaxKind;
use ridl_syntax::ast::{self, AstNode, Definition, HasDocComments, HasModifiers};
use rowan::{NodeOrToken, TextRange};

use crate::docs;
use crate::init;
use crate::resolve::{
    Resolution, Symbol, SymbolKind, declared_name, declared_symbols, name_range,
    qualified_segments, resolve_package, significant_text, source_file,
};
use crate::scalar::{
    DiagKind, ExactValue, FloatRange, IntRange, derive_float_width, derive_int_width,
    enumset_width, validate_range, validate_step,
};
use crate::ucum::parse_ucum;

/// A checked package: its lowered IR and the checker's own diagnostics.
///
/// The diagnostics' spans carry a [`FileId`] indexing `pkg.files(db)` in order
/// — the same package-relative scheme [`Resolution`] uses. The resolver's
/// diagnostics are not repeated here; a renderer reads both and remaps them
/// onto its own source map with [`ridl_core::diag::remap_diagnostics`].
#[derive(Debug, Clone, PartialEq)]
pub struct CheckedPackage {
    pub ir: v1::Package,
    pub diagnostics: Vec<Diagnostic>,
}

/// Checks `pkg` and lowers it to IR v1 (typl reference §4–§12, §16.2–§16.3).
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

    let mut checker = Checker {
        db,
        ws,
        std,
        pkg,
        package_name: package_name.clone(),
        resolution,
        file_ids,
        current_file: 0,
        diagnostics: Vec::new(),
    };

    let mut decls = Vec::new();
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
    }

    checker.check_recursion(&composite_starts);

    CheckedPackage {
        ir: v1::Package {
            name: package_name,
            decls,
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

struct Checker<'db> {
    db: &'db dyn salsa::Database,
    ws: Workspace,
    std: Package,
    pkg: Package,
    package_name: String,
    resolution: Resolution,
    file_ids: Vec<FileId>,
    current_file: usize,
    diagnostics: Vec<Diagnostic>,
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
    constraint: Option<v1::Constraint>,
    width: Option<v1::type_def::Width>,
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
    ty: v1::FieldType,
    /// Exact numeric bounds when the field type is a numeric scalar, used to
    /// validate a numeric declared init (TYPL-109).
    scalar_bounds: Option<(Option<ExactValue>, Option<ExactValue>)>,
    /// Length bounds and `match` pattern when the field type is a string/bytes
    /// scalar — inline (`name : string [0..8]`) or a named string/bytes `type`.
    /// Used to validate a declared string/bytes init (TYPL-109); the numeric
    /// path reads `scalar_bounds` instead.
    init_constraint: Option<v1::Constraint>,
}

impl LoweredType {
    fn plain(ty: v1::FieldType) -> Self {
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

    fn warning(&mut self, code: DiagCode, range: TextRange, message: String) {
        self.diag(code, Severity::Warning, range, message);
    }

    fn info(&mut self, code: DiagCode, range: TextRange, message: String) {
        self.diag(code, Severity::Info, range, message);
    }

    /// Whether `definition` is the resolver's first-wins winner for its name —
    /// the one occurrence the checker lowers (ADR-0007 decision 6).
    fn is_winner(&self, file: InputFile, definition: &Definition) -> bool {
        let Some(name) = declared_name(definition) else {
            return false;
        };
        match self.resolution.symbols.get(&name) {
            Some(symbol) => {
                symbol.package == self.package_name
                    && symbol.file == file
                    && symbol.range == name_range(definition)
            }
            None => false,
        }
    }

    /// The package handle for a package name: the checked package itself, the
    /// embedded `ridl.std`, or a workspace member.
    fn package_handle(&self, name: &str) -> Option<Package> {
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
    fn canonical_ref(&self, symbol: &Symbol) -> String {
        if symbol.package == self.package_name {
            symbol.name.clone()
        } else {
            format!("{}.{}", symbol.package, symbol.name)
        }
    }

    /// Resolves a type path, emitting the T6 description-first messages
    /// (unknown name / const-where-type-expected) — no §16 code exists for
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
    fn lookup_path(&self, path: &ast::PathType) -> Option<Symbol> {
        self.lookup_path_in(&self.resolution, path)
    }

    /// Resolves a path silently in a given package view: a single segment is a
    /// bare name in that view; a longer path is a fully qualified
    /// `pkg.Name` reference (typl §3.2 — no import needed).
    fn lookup_path_in(&self, resolution: &Resolution, path: &ast::PathType) -> Option<Symbol> {
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
    fn find_definition(&self, symbol: &Symbol) -> Option<Definition> {
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

    fn lower_definition(&mut self, definition: &Definition) -> Option<v1::Decl> {
        let name = declared_name(definition)?;
        let kind = match definition {
            Definition::Type(decl) => v1::decl::Kind::TypeDef(self.lower_type(&name, decl)),
            Definition::Const(decl) => v1::decl::Kind::ConstDef(self.lower_const(&name, decl)),
            Definition::Struct(decl) => v1::decl::Kind::StructDef(self.lower_struct(decl)),
            Definition::Enum(decl) => v1::decl::Kind::EnumDef(self.lower_enum(decl)),
            Definition::EnumSet(decl) => v1::decl::Kind::EnumSetDef(self.lower_enum_set(decl)),
            Definition::Union(decl) => v1::decl::Kind::UnionDef(self.lower_union(decl)),
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
            v1::Visibility::Internal
        } else {
            v1::Visibility::Public
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

        // TYPL-005: a public declaration must not expose an `internal` type.
        self.check_internal_exposure(definition, &name);

        Some(v1::Decl {
            name,
            visibility: visibility as i32,
            is_error: definition.is_error(),
            doc: doc_info.doc,
            labels: doc_info.labels,
            deprecated: doc_info.deprecated,
            kind: Some(kind),
        })
    }

    /// TYPL-005: a public declaration must not expose an `internal` type in its
    /// fields, arms, backing, or range-bound constants (typl §3.3). Only
    /// same-package internal declarations are reachable — a foreign `internal`
    /// name never resolves (typl §3.3), so it cannot leak here. Internal
    /// declarations are exempt: they may reference each other freely.
    fn check_internal_exposure(&mut self, definition: &Definition, decl_name: &str) {
        if definition.is_internal() {
            return;
        }
        // Collect exposures first (immutable resolver reads), then report.
        let mut exposures: Vec<(TextRange, &'static str, String)> = Vec::new();
        for descendant in definition.syntax().descendants() {
            // A named-type reference: field, arm, map key/value, or an enumset
            // backing enum.
            if let Some(path) = ast::PathType::cast(descendant.clone()) {
                if let Some(symbol) = self.lookup_path(&path)
                    && symbol.internal
                    && symbol.package == self.package_name
                {
                    exposures.push((path.syntax().text_range(), "type", symbol.name.clone()));
                }
                continue;
            }
            // A range-bound or `match` constant: an `Ident` inside a `Literal`
            // sitting in a `Constraint` (never an init value).
            if let Some(literal) = ast::Literal::cast(descendant)
                && literal
                    .syntax()
                    .ancestors()
                    .any(|ancestor| ancestor.kind() == SyntaxKind::Constraint)
                && let LitKind::ConstRef(const_name) = literal_kind(&literal)
                && let Some(symbol) = self.resolution.symbols.get(&const_name)
                && symbol.internal
                && symbol.package == self.package_name
            {
                exposures.push((
                    literal.syntax().text_range(),
                    "constant",
                    symbol.name.clone(),
                ));
            }
        }
        for (range, noun, exposed) in exposures {
            self.error(
                DiagCode::TYPL_005,
                range,
                format!("public `{decl_name}` exposes internal {noun} `{exposed}`"),
            );
        }
    }

    fn lower_type(&mut self, name: &str, decl: &ast::TypeDef) -> v1::TypeDef {
        let (backing, class) = self.lower_backing(decl.backing());
        let span = decl
            .backing()
            .map(|backing| backing.syntax().text_range())
            .unwrap_or_else(|| name_range(decl));
        let parts = self.lower_scalar(class, decl.constraint(), span);
        let (declared_init, declared) = self.lower_declared_init(decl.init_value(), &parts);
        let mut type_def = v1::TypeDef {
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
    ) -> (Option<v1::Backing>, BackingClass) {
        match backing {
            None => (None, BackingClass::Unknown),
            Some(ast::Backing::Primitive(node)) => {
                let (primitive, class) = primitive_of(&node);
                (
                    Some(v1::Backing {
                        kind: Some(v1::backing::Kind::Primitive(primitive as i32)),
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
                    Some(v1::Backing {
                        kind: Some(v1::backing::Kind::Unit(unit)),
                    }),
                    BackingClass::Float,
                )
            }
        }
    }

    fn lower_const(&mut self, name: &str, decl: &ast::ConstDef) -> v1::ConstDef {
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
        v1::ConstDef {
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
                width: Some(v1::type_def::Width::IntWidth(v1::IntWidth::I64 as i32)),
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
                Ok(width) => Some(v1::type_def::Width::IntWidth(
                    v1::IntWidth::from(width) as i32
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
            constraint: Some(v1::Constraint {
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
                width: Some(v1::type_def::Width::FloatWidth(v1::FloatWidth::F64 as i32)),
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
            Some(v1::type_def::Width::FloatWidth(
                v1::FloatWidth::from(width) as i32
            ))
        };
        ScalarParts {
            constraint: Some(v1::Constraint {
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
            constraint: Some(v1::Constraint {
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
    fn lower_declared_init(
        &mut self,
        init: Option<ast::InitValue>,
        parts: &ScalarParts,
    ) -> (Option<String>, Option<v1::InitValue>) {
        let Some(literal) = init.as_ref().and_then(ast::InitValue::literal) else {
            return (None, None);
        };
        let text = match literal_kind(&literal) {
            LitKind::Number { value } => {
                if out_of_bounds(&value, parts.min.as_ref(), parts.max.as_ref()) {
                    self.error(
                        DiagCode::TYPL_109,
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
                self.check_string_init(&text, parts, literal.syntax().text_range());
                text
            }
            LitKind::ConstRef(name) => match self.const_numeric_value_in(self.pkg, &name) {
                Some(value) => {
                    if out_of_bounds(&value, parts.min.as_ref(), parts.max.as_ref()) {
                        self.error(
                            DiagCode::TYPL_109,
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
                None => significant_text(literal.syntax()),
            },
            LitKind::Regex(text) => text,
            LitKind::Malformed => significant_text(literal.syntax()),
        };
        (
            Some(text.clone()),
            Some(v1::InitValue {
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
    fn check_string_init(&mut self, text: &str, parts: &ScalarParts, range: TextRange) {
        let Some(constraint) = &parts.constraint else {
            return;
        };
        let length = text.chars().count() as u64;
        if let Some(min) = constraint.len_min
            && length < min
        {
            self.error(
                DiagCode::TYPL_109,
                range,
                format!("init string length {length} is below the declared minimum {min}"),
            );
        } else if let Some(max) = constraint.len_max
            && length > max
        {
            self.error(
                DiagCode::TYPL_109,
                range,
                format!("init string length {length} exceeds the declared maximum {max}"),
            );
        }
        if let Some(pattern) = &constraint.pattern
            && let Ok(regex) = regress::Regex::new(regex_body(pattern))
            && regex.find(text).is_none()
        {
            self.error(
                DiagCode::TYPL_109,
                range,
                format!("init string `{text}` does not match the type's `match` pattern"),
            );
        }
    }

    // --- structs ----------------------------------------------------------

    fn lower_struct(&mut self, decl: &ast::StructDef) -> v1::StructDef {
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
                    members.push(v1::StructMember {
                        member: Some(v1::struct_member::Member::Reserved(lower_reserved(
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
                    members.push(v1::StructMember {
                        member: Some(v1::struct_member::Member::Field(lowered)),
                    });
                }
            }
        }
        v1::StructDef {
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

    fn lower_field(&mut self, field: &ast::FieldDef, ordinal: u32) -> v1::Field {
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
        let (declared_init, declared) = self.lower_declared_init(field.init_value(), &bounds_parts);
        // E1.9: a field without a declared init derives one from the §5.8 table
        // (a named reference resolves to the referenced type's own init).
        let init = match declared {
            Some(init) => Some(init),
            None => lowered.as_ref().map(|l| self.derive_field_init(&l.ty)),
        };
        v1::Field {
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
    fn derive_field_init(&self, field_type: &v1::FieldType) -> v1::InitValue {
        init::derive_field_init(field_type, &|name| self.named_ref_init(name))
    }

    /// The derived init of a named reference (`Speed`, `ridl.std.Name`), by kind
    /// (typl §5.8): a scalar `type` materializes its value; an `enum` its `0`
    /// or lowest value; an `enumset` the empty set; a `struct` or `union` is a
    /// derivable composite the consumer reconstructs (a `union` inherits its
    /// first arm's derivability). An unresolved reference — already reported by
    /// the type-resolution pass — is treated as a derivable composite.
    fn named_ref_init(&self, canonical: &str) -> v1::InitValue {
        match self.resolve_canonical(canonical) {
            Some(symbol) => self.named_type_init(&symbol),
            None => v1::InitValue {
                derivable: true,
                value: None,
            },
        }
    }

    /// The derived init of a resolved named type (typl §5.8).
    fn named_type_init(&self, symbol: &Symbol) -> v1::InitValue {
        match symbol.kind {
            SymbolKind::Type => {
                let Some(Definition::Type(decl)) = self.find_definition(symbol) else {
                    return v1::InitValue {
                        derivable: true,
                        value: None,
                    };
                };
                // A named type with its own declared init carries that value.
                if let Some(init) = self.declared_type_init(&decl, symbol) {
                    return init;
                }
                match backing_class(decl.backing()) {
                    BackingClass::Boolean => v1::InitValue {
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
                    BackingClass::Unknown => v1::InitValue {
                        derivable: true,
                        value: None,
                    },
                }
            }
            SymbolKind::Enum => self.enum_default_init(symbol),
            SymbolKind::EnumSet => v1::InitValue {
                // The empty set — no bits set (typl §5.8).
                derivable: true,
                value: Some(String::new()),
            },
            SymbolKind::Struct => v1::InitValue {
                derivable: true,
                value: None,
            },
            SymbolKind::Union => v1::InitValue {
                derivable: self.union_is_derivable(symbol),
                value: None,
            },
            SymbolKind::Const => v1::InitValue {
                derivable: false,
                value: None,
            },
        }
    }

    /// The materialized value of a named type's own declared `= value` init
    /// (typl §5.8), or `None` when the type declares no init. A constant-valued
    /// init resolves through the const chain in the type's defining package.
    fn declared_type_init(&self, decl: &ast::TypeDef, symbol: &Symbol) -> Option<v1::InitValue> {
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
        Some(v1::InitValue {
            derivable: true,
            value: Some(value),
        })
    }

    /// The derived enum init (typl §5.8): the value `0` when an enum value
    /// declares it, otherwise the lowest declared value. A degenerate enum with
    /// no integer-valued members is not derivable.
    fn enum_default_init(&self, symbol: &Symbol) -> v1::InitValue {
        let Some(Definition::Enum(decl)) = self.find_definition(symbol) else {
            return v1::InitValue {
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
            Some(_) if values.contains(&0) => v1::InitValue {
                derivable: true,
                value: Some("0".to_string()),
            },
            Some(lowest) => v1::InitValue {
                derivable: true,
                value: Some(lowest.to_string()),
            },
            None => v1::InitValue {
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
    fn named_string_constraint(&self, symbol: &Symbol) -> Option<v1::Constraint> {
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
        Some(v1::Constraint {
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
                        LoweredType::plain(v1::FieldType {
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
                    .map(|field| v1::TupleField {
                        name: member_name(field.name()).unwrap_or_default(),
                        r#type: field
                            .field_type()
                            .map(|inner| self.lower_field_type(&inner, false).ty),
                    })
                    .collect();
                LoweredType::plain(v1::FieldType {
                    optional: false,
                    kind: Some(v1::field_type::Kind::Tuple(v1::TupleType { fields })),
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
                LoweredType::plain(v1::FieldType {
                    optional: false,
                    kind: Some(v1::field_type::Kind::Array(Box::new(v1::ArrayType {
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
                LoweredType::plain(v1::FieldType {
                    optional: false,
                    kind: Some(v1::field_type::Kind::Map(Box::new(v1::MapType {
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
                    ty: v1::FieldType {
                        optional: false,
                        kind: Some(v1::field_type::Kind::Named(self.canonical_ref(&symbol))),
                    },
                    scalar_bounds,
                    init_constraint,
                }
            }
            PathTarget::Unresolved(written) => LoweredType::plain(v1::FieldType {
                optional: false,
                kind: Some(v1::field_type::Kind::Named(written)),
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
                    ty: v1::FieldType {
                        optional: false,
                        kind: Some(v1::field_type::Kind::InlineScalar(Box::new(v1::TypeDef {
                            backing: Some(v1::Backing {
                                kind: Some(v1::backing::Kind::Primitive(primitive as i32)),
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
                LoweredType::plain(v1::FieldType {
                    optional: false,
                    kind: Some(v1::field_type::Kind::Primitive(primitive as i32)),
                })
            }
        }
    }

    /// Lowers a map key, enforcing the §12.2 key shape: a named string type
    /// or a primitive (TYPL-209).
    fn lower_map_key(&mut self, key: &ast::FieldType) -> v1::FieldType {
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
                    v1::FieldType {
                        optional: false,
                        kind: Some(v1::field_type::Kind::Named(self.canonical_ref(&symbol))),
                    }
                }
                PathTarget::Unresolved(written) => v1::FieldType {
                    optional: false,
                    kind: Some(v1::field_type::Kind::Named(written)),
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

    fn lower_enum(&mut self, decl: &ast::EnumDef) -> v1::EnumDef {
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
            values.push(v1::EnumValue {
                name,
                value,
                doc: String::new(),
            });
        }
        v1::EnumDef { values, reserved }
    }

    fn lower_enum_set(&mut self, decl: &ast::EnumSetDef) -> v1::EnumSetDef {
        let mut backing_enum = None;
        let mut bits: Vec<v1::EnumValue> = Vec::new();

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
                            bits.push(v1::EnumValue {
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
                bits.push(v1::EnumValue {
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
        v1::EnumSetDef {
            backing_enum,
            bits,
            width: v1::IntWidth::from(enumset_width(highest)) as i32,
        }
    }

    // --- unions -----------------------------------------------------------

    fn lower_union(&mut self, decl: &ast::UnionDef) -> v1::UnionDef {
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
            arms.push(v1::UnionArm {
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

        v1::UnionDef {
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

    // --- fixed-width analysis ---------------------------------------------

    /// Whether a field type is fixed-width in the given package view.
    /// `visiting` breaks reference cycles (a recursive shape is TYPL-206
    /// elsewhere; here it is simply not fixed).
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
                    BackingClass::Str | BackingClass::Bytes => node
                        .constraint()
                        .and_then(|constraint| constraint.len())
                        .is_some_and(|bound| self.bound_is_fixed(&bound)),
                    BackingClass::Unknown => false,
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
            SymbolKind::Union | SymbolKind::Const => false,
            SymbolKind::Type => match self.find_definition(symbol) {
                Some(Definition::Type(decl)) => match decl.backing() {
                    Some(ast::Backing::Unit(_)) => true,
                    Some(ast::Backing::Primitive(node)) => {
                        let (_, class) = primitive_of(&node);
                        match class {
                            BackingClass::Boolean | BackingClass::Integer | BackingClass::Float => {
                                true
                            }
                            BackingClass::Str | BackingClass::Bytes => decl
                                .constraint()
                                .and_then(|constraint| constraint.len())
                                .is_some_and(|bound| self.bound_is_fixed(&bound)),
                            BackingClass::Unknown => false,
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
fn member_name(name: Option<ast::Name>) -> Option<String> {
    Some(name?.ident_token()?.text().to_string())
}

/// The source range of a member's name, or the whole node on a malformed
/// tree.
fn member_name_range(name: Option<ast::Name>, node: &ridl_syntax::SyntaxNode) -> TextRange {
    match name {
        Some(name) => name.syntax().text_range(),
        None => node.text_range(),
    }
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
fn primitive_of(node: &ast::PrimitiveType) -> (v1::PrimitiveType, BackingClass) {
    if node.boolean_token().is_some() {
        (v1::PrimitiveType::Boolean, BackingClass::Boolean)
    } else if node.integer_token().is_some() {
        (v1::PrimitiveType::Integer, BackingClass::Integer)
    } else if node.float_token().is_some() {
        (v1::PrimitiveType::Float, BackingClass::Float)
    } else if node.string_token().is_some() {
        (v1::PrimitiveType::String, BackingClass::Str)
    } else if node.bytes_token().is_some() {
        (v1::PrimitiveType::Bytes, BackingClass::Bytes)
    } else {
        (v1::PrimitiveType::Unspecified, BackingClass::Unknown)
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

fn lower_reserved(entry: &ast::ReservedEntry, ordinal: u32) -> v1::Reserved {
    let name = member_name(entry.name());
    let value = entry
        .literal()
        .and_then(|literal| match literal_kind(&literal) {
            LitKind::Number { value } => exact_to_i64(&value),
            _ => None,
        });
    v1::Reserved {
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
    }
}

fn kind_article(kind: SymbolKind) -> &'static str {
    match kind {
        SymbolKind::Enum | SymbolKind::EnumSet => "an",
        _ => "a",
    }
}

fn out_of_bounds(value: &ExactValue, min: Option<&ExactValue>, max: Option<&ExactValue>) -> bool {
    min.is_some_and(|min| value.0 < min.0) || max.is_some_and(|max| value.0 > max.0)
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
    use ridl_core::package::PackageOrigin;
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
    fn decl<'a>(checked: &'a CheckedPackage, name: &str) -> &'a v1::Decl {
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

    fn type_def<'a>(checked: &'a CheckedPackage, name: &str) -> &'a v1::TypeDef {
        let Some(v1::decl::Kind::TypeDef(def)) = &decl(checked, name).kind else {
            panic!("`{name}` is not a type def");
        };
        def
    }

    fn struct_def<'a>(checked: &'a CheckedPackage, name: &str) -> &'a v1::StructDef {
        let Some(v1::decl::Kind::StructDef(def)) = &decl(checked, name).kind else {
            panic!("`{name}` is not a struct def");
        };
        def
    }

    fn union_def<'a>(checked: &'a CheckedPackage, name: &str) -> &'a v1::UnionDef {
        let Some(v1::decl::Kind::UnionDef(def)) = &decl(checked, name).kind else {
            panic!("`{name}` is not a union def");
        };
        def
    }

    // --- the Appendix B golden -------------------------------------------

    /// The full Appendix B package lowers end to end with no diagnostics, and
    /// its IR v1 JSON is pinned as the reviewed golden snapshot.
    #[test]
    fn appendix_b_lowers_clean_end_to_end() {
        let checked = check_source("veh.common", APPENDIX_B);
        assert!(
            checked.diagnostics.is_empty(),
            "Appendix B must lower clean, got: {:?}",
            checked.diagnostics,
        );
        assert_eq!(checked.ir.name, "veh.common");
        insta::assert_snapshot!("appendix_b_ir", v1::to_json_pretty(&checked.ir));
    }

    /// `fixed_layout` is derived per struct: every field fixed-width and
    /// non-optional, and no tombstone (typl Appendix D, FlatBuffers note).
    #[test]
    fn appendix_b_fixed_layout_is_derived() {
        let checked = check_source("veh.common", APPENDIX_B);
        assert!(struct_def(&checked, "SpeedLimitPayload").fixed_layout);
        assert!(struct_def(&checked, "SensorReading").fixed_layout);
        assert!(struct_def(&checked, "RawWheelFrame").fixed_layout);
        // DriverProfile has an optional field; SensorFault holds a
        // variable-length Message; SensorBounds holds bounded collections.
        assert!(!struct_def(&checked, "DriverProfile").fixed_layout);
        assert!(!struct_def(&checked, "SensorFault").fixed_layout);
        assert!(!struct_def(&checked, "SensorBounds").fixed_layout);
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
        let Some(v1::decl::Kind::TypeDef(def)) = &speeds[0].kind else {
            panic!("Speed is a type def");
        };
        assert_eq!(
            def.backing.as_ref().unwrap().kind,
            Some(v1::backing::Kind::Unit("km/h".to_string())),
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
    /// `unit: "integer"` artifact must not carry into IR v1.
    #[test]
    fn primitive_backing_lowers_to_backing_primitive() {
        let checked = check_source("app", "package app\ntype Counter : integer [0..65535]\n");
        let def = type_def(&checked, "Counter");
        assert_eq!(
            def.backing.as_ref().unwrap().kind,
            Some(v1::backing::Kind::Primitive(
                v1::PrimitiveType::Integer as i32
            )),
        );
        assert_eq!(
            def.width,
            Some(v1::type_def::Width::IntWidth(v1::IntWidth::U16 as i32)),
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
            Some(v1::backing::Kind::Unit("km/h".to_string())),
        );
        assert_eq!(
            def.width,
            Some(v1::type_def::Width::FloatWidth(v1::FloatWidth::F32 as i32)),
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
            Some(v1::type_def::Width::IntWidth(v1::IntWidth::I64 as i32)),
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
            Some(v1::type_def::Width::FloatWidth(v1::FloatWidth::F64 as i32)),
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
        let Some(v1::decl::Kind::ConstDef(def)) = &decl(&checked, "TOO_FAST").kind else {
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
            Some(v1::InitValue {
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
            Some(v1::InitValue {
                derivable: true,
                value: Some("0".to_string()),
            }),
        );
        // Gain declares no init, so it derives `0` (within `[0.0..1.0]`) —
        // rendered as the canonical `0` — with `declared_init` still absent.
        assert_eq!(
            type_def(&checked, "Gain").init,
            Some(v1::InitValue {
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
            Some(v1::type_def::Width::IntWidth(v1::IntWidth::U8 as i32)),
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
            Some(v1::type_def::Width::IntWidth(v1::IntWidth::U64 as i32)),
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
        let Some(v1::decl::Kind::EnumSetDef(def)) = &decl(&checked, "WarningFlags").kind else {
            panic!("WarningFlags is an enumset def");
        };
        assert_eq!(def.backing_enum.as_deref(), Some("Warning"));
        assert_eq!(
            def.bits.iter().map(|b| b.value).collect::<Vec<_>>(),
            vec![0, 1],
            "the derived form copies the backing enum's values",
        );
        assert_eq!(def.width, v1::IntWidth::U8 as i32);
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
        let v1::struct_member::Member::Field(field) = struct_def(&checked, "S").members[0]
            .member
            .as_ref()
            .unwrap()
        else {
            panic!("expected a field");
        };
        let Some(v1::field_type::Kind::InlineScalar(inline)) = &field.r#type.as_ref().unwrap().kind
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
        let v1::struct_member::Member::Field(name_field) = members[0].member.as_ref().unwrap()
        else {
            panic!("member 1 is a field");
        };
        assert_eq!((name_field.name.as_str(), name_field.ordinal), ("name", 1));
        let v1::struct_member::Member::Reserved(tombstone) = members[1].member.as_ref().unwrap()
        else {
            panic!("member 2 is reserved");
        };
        assert_eq!(tombstone.ordinal, 2);
        assert_eq!(tombstone.name.as_deref(), Some("legacyChecksum"));
        let v1::struct_member::Member::Field(speed_field) = members[2].member.as_ref().unwrap()
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
            Some(v1::InitValue {
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
    fn iv(derivable: bool, value: Option<&str>) -> v1::InitValue {
        v1::InitValue {
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
    ) -> Option<v1::InitValue> {
        struct_def(checked, struct_name)
            .members
            .iter()
            .find_map(|member| match member.member.as_ref()? {
                v1::struct_member::Member::Field(field) if field.name == field_name => {
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
}
