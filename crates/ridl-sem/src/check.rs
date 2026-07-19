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
//! - init-value derivation is task 15: this pass lowers the **declared**
//!   `= value` init (validated as TYPL-109) into `InitValue`; a declaration
//!   without one carries no `InitValue` until the task 15 derivation pass
//!   fills it in.

use std::collections::HashSet;

use ridl_core::db::InputFile;
use ridl_core::diag::{DiagCode, Diagnostic, FileId, Severity, SourceMap, Span};
use ridl_core::package::{Package, Workspace, package_of};
use ridl_ir::v1;
use ridl_syntax::SyntaxKind;
use ridl_syntax::ast::{self, AstNode, Definition, HasModifiers};
use rowan::TextRange;

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
    scalar_bounds: Option<(Option<ExactValue>, Option<ExactValue>)>,
}

impl LoweredType {
    fn plain(ty: v1::FieldType) -> Self {
        LoweredType {
            ty,
            scalar_bounds: None,
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

    /// The numeric value of a constant, resolved in the context of `package`
    /// (one level — a const referencing another const is task 14 territory).
    fn const_numeric_value_in(&self, package: Package, name: &str) -> Option<ExactValue> {
        let resolution = resolve_package(self.db, self.ws, package, self.std);
        let symbol = resolution.symbols.get(name)?.clone();
        if symbol.kind != SymbolKind::Const {
            return None;
        }
        let Definition::Const(decl) = self.find_definition(&symbol)? else {
            return None;
        };
        match literal_kind(&decl.value()?) {
            LitKind::Number { value } => Some(value),
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
    /// a number directly, or a named constant resolved one level.
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

    // --- declarations -----------------------------------------------------

    fn lower_definition(&mut self, definition: &Definition) -> Option<v1::Decl> {
        let name = declared_name(definition)?;
        let kind = match definition {
            Definition::Type(decl) => v1::decl::Kind::TypeDef(self.lower_type(decl)),
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
        Some(v1::Decl {
            name,
            visibility: visibility as i32,
            is_error: definition.is_error(),
            // Doc semantics (body, @labels, @deprecated) land with the task 14
            // doc scanner.
            doc: String::new(),
            labels: Vec::new(),
            deprecated: None,
            kind: Some(kind),
        })
    }

    fn lower_type(&mut self, decl: &ast::TypeDef) -> v1::TypeDef {
        let (backing, class) = self.lower_backing(decl.backing());
        let span = decl
            .backing()
            .map(|backing| backing.syntax().text_range())
            .unwrap_or_else(|| name_range(decl));
        let parts = self.lower_scalar(class, decl.constraint(), span);
        let (declared_init, init) = self.lower_declared_init(decl.init_value(), &parts);
        v1::TypeDef {
            backing,
            constraint: parts.constraint,
            declared_init,
            init,
            width: parts.width,
        }
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

        // The declared type: a primitive keyword as written, or a named-type
        // reference in canonical form. Absent for regex constants.
        let mut bounds: Option<(Option<ExactValue>, Option<ExactValue>, String)> = None;
        let type_ref = decl.type_ref().map(|path| {
            if let Some(keyword) = primitive_path_keyword(&path) {
                return keyword;
            }
            match self.resolve_type_path(&path) {
                PathTarget::Symbol(symbol) => {
                    let canonical = self.canonical_ref(&symbol);
                    if symbol.kind == SymbolKind::Type
                        && let Some((min, max)) = self.named_scalar_bounds(&symbol)
                    {
                        bounds = Some((min, max, canonical.clone()));
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
            let range = value_literal
                .as_ref()
                .map(|literal| literal.syntax().text_range())
                .unwrap_or_default();
            self.error(
                DiagCode::TYPL_108,
                range,
                format!(
                    "const `{name}` value {} outside `{type_name}` range [{}, {}]",
                    value.to_decimal_string(),
                    render_bound(min.as_ref()),
                    render_bound(max.as_ref()),
                ),
            );
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
        let min = constraint
            .min()
            .and_then(|literal| self.numeric_literal(&literal));
        let max = constraint
            .max()
            .and_then(|literal| self.numeric_literal(&literal));

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

        // §5.5: an omitted bound defaults to the widest value the inferred
        // width allows — the int64 domain edge on the open side.
        let effective_min = min.clone().unwrap_or_else(|| int64_edge(false));
        let effective_max = max.clone().unwrap_or_else(|| int64_edge(true));
        let width = match derive_int_width(&IntRange {
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
                    "integer range bound outside the `int64` domain `[-2^63..2^63-1]`".to_string(),
                );
                None
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
        let min = constraint
            .min()
            .and_then(|literal| self.numeric_literal(&literal));
        let max = constraint
            .max()
            .and_then(|literal| self.numeric_literal(&literal));

        let step_literal = constraint.step();
        let step = step_literal
            .as_ref()
            .and_then(|literal| self.numeric_literal(literal));
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

        // §4 recommendation: a float wants both a range and a step.
        if min.is_none() || max.is_none() || step.is_none() {
            missing_constraint_warning(self, decl_span);
        }

        let width = if min.is_some() && max.is_some() {
            derive_float_width(&float_range(step.clone()))
        } else {
            crate::scalar::FloatWidth::F64
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
            width: Some(v1::type_def::Width::FloatWidth(
                v1::FloatWidth::from(width) as i32
            )),
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
                LitKind::Regex(text) => pattern = Some(text),
                LitKind::ConstRef(name) => match self.const_regex_value(&name) {
                    Some((text, canonical)) => {
                        pattern = Some(text);
                        pattern_const = Some(canonical);
                    }
                    // An unresolved or non-regex pattern constant: TYPL-106
                    // (regex validation) is task 14 scope; carry the name.
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
    /// bounds (TYPL-109). Returns `(declared_init, init)`; both stay absent
    /// when no init is declared — derivation is the task 15 pass.
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
            LitKind::Str(text) => text,
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
        let bounds_parts = ScalarParts {
            constraint: None,
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
        let (declared_init, init) = self.lower_declared_init(field.init_value(), &bounds_parts);
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
                LoweredType {
                    ty: v1::FieldType {
                        optional: false,
                        kind: Some(v1::field_type::Kind::Named(self.canonical_ref(&symbol))),
                    },
                    scalar_bounds,
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
            let mut kind_index = 0;
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
                kind_index += 1;
                let _ = kind_index;
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

    /// A valid declared init lowers into `InitValue`; a declaration without
    /// one carries no `InitValue` until the task 15 derivation pass.
    #[test]
    fn declared_init_lowers_and_underived_init_stays_unset() {
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
        assert_eq!(type_def(&checked, "Gain").init, None);
        assert_eq!(type_def(&checked, "Gain").declared_init, None);
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
        let found = codes(&checked);
        assert!(
            found.iter().filter(|code| **code == "TYPL-206").count() >= 1,
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
}
