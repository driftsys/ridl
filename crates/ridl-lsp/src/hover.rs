//! Hover content (ADR-0004 §10).
//!
//! Hover on a type reference or a declaration renders the declaration's IR —
//! qualified name, kind, backing and canonical UCUM unit, constraint, derived
//! wire width, init value, doc comment, labels, and deprecation — pulled from
//! the [`CheckedPackage`](ridl_sem::CheckedPackage) IR, the richest source,
//! keyed by the symbol [`symbol_at`] resolves. Hover on a
//! struct field instead shows the field's derived ordinal (general form §6.3
//! groundwork; the ordinal is typl §7.4), read straight from the IR so it counts
//! reserved tombstones exactly as codegen does.
//!
//! Every hover shows, in order (ADR-0026): the signature, the doc comment
//! with each doc link rendered as a Markdown link to the target's location
//! ([`render_doc`]), a **Contract** list from [`ridl_ir::rules`], and the
//! `@since` versions, the labels and the deprecation reason. An enum value,
//! an enumset bit, a union arm and a parameter have their own hover; a
//! parameter with no doc shows the doc of its named type.
//!
//! The ridl layer adds three more anchors, all read from the same
//! checked IR so the editor can never disagree with codegen:
//!
//! - **An interaction** renders its kind, resolved payload, §11 ordinal, and —
//!   for a signal or an event — its *resolved* timing: mode, bounds, and the
//!   per-kind reading general form §6.2 derives from the declaring keyword
//!   rather than from the annotation. A state value that arrives late is
//!   refreshed and a fast one debounced; a stale occurrence is discarded and a
//!   fast one throttled. When the interaction carried no annotation the
//!   configured default was resolved at compile time, and the hover says so —
//!   "untimed" does not exist past the parser (ridl §9.1).
//! - **A fallible return** (`T | E`, general form §6.1) names both arms and
//!   closes with the ridl §10 strata note. Stratum 3 is never called undefined
//!   behavior: the runtime detects those failures, the contract language merely
//!   does not declare them (general form §6.4).
//! - **A service** names its interface shape — or reports its inline one — and
//!   states the ridl §14.5 posture neutrality, because a service declaration
//!   says nothing about how it is realized on the wire.

use std::collections::HashMap;

use lsp_types as lt;
use ridl_core::db::InputFile;
use ridl_core::package::{Package, Workspace, package_of};
use ridl_ir::rules::{self, ItemRef, Rule};
use ridl_ir::v2;
use ridl_sem::{Symbol, SymbolKind, check_package, resolve_doc_link, resolve_package};
use ridl_syntax::ast::{
    AstNode, Expr, HasName, InterfaceDef, InterfaceMember, MemberExpr, ServiceDef,
};
use ridl_syntax::{SyntaxKind, SyntaxNode};
use rowan::{TextRange, TextSize};

use crate::convert;
use crate::nav::{self, symbol_at};

/// Rendered hover content plus the source range it describes.
#[derive(Debug, Clone)]
pub struct HoverInfo {
    /// CommonMark markdown for the LSP hover popup.
    pub markdown: String,
    /// The reference or name span the hover is anchored to.
    pub range: TextRange,
}

/// Builds the hover for the cursor at `offset` in `file` (a file of `pkg`), or
/// `None` when the cursor is not on something with hover content.
pub fn hover(
    db: &dyn salsa::Database,
    ws: Workspace,
    std: Package,
    pkg: Package,
    file: InputFile,
    offset: TextSize,
) -> Option<HoverInfo> {
    let scope = Scope { db, ws, std, pkg };
    // A doc link shows its target's hover (ADR-0026).
    if let Some(info) = doc_link_hover(scope, file, offset) {
        return Some(info);
    }
    // A member name (a field, an enum value, an enumset bit, a union arm or a
    // parameter) is not a symbol — resolve that first.
    if let Some(info) = member_hover(scope, file, offset) {
        return Some(info);
    }
    if let Some(info) = contract_use_hover(scope, file, offset) {
        return Some(info);
    }
    // Interaction and service anchors are not symbols either: an interaction
    // name lives inside an interface body and a service name in the workspace
    // catalog, so `symbol_at` would return `None` for both.
    if let Some(info) = interaction_hover(db, ws, std, pkg, file, offset) {
        return Some(info);
    }
    if let Some(info) = service_hover(db, ws, pkg, std, file, offset) {
        return Some(info);
    }

    let located = symbol_at(db, ws, std, pkg, file, offset)?;
    let markdown = symbol_markdown(db, ws, std, pkg, &located.symbol);
    Some(HoverInfo {
        markdown,
        range: located.reference,
    })
}

/// The packages and database a hover reads from: the cursor's package `pkg`,
/// the workspace, and the embedded `ridl.std`.
#[derive(Clone, Copy)]
struct Scope<'db> {
    db: &'db dyn salsa::Database,
    ws: Workspace,
    std: Package,
    pkg: Package,
}

impl Scope<'_> {
    /// The location a doc link to the canonical `target` points at.
    fn resolve(&self, target: &str) -> Option<lt::Location> {
        link_location(self.db, self.ws, self.std, self.pkg, target)
    }

    /// The IR of the package named `name`.
    fn package_ir(&self, name: &str) -> Option<v2::Package> {
        let package = owner_package(self.db, self.ws, self.std, self.pkg, name)?;
        Some(check_package(self.db, self.ws, package, self.std).ir)
    }

    /// The IR of the package a named field or parameter type comes from, when
    /// that is not the cursor's package: the `deps` of [`rules::rules`].
    fn deps_of(&self, field_type: Option<&v2::FieldType>) -> Vec<v2::Package> {
        match field_type
            .and_then(named_ref)
            .and_then(|name| name.rsplit_once('.'))
        {
            Some((package, _)) if package != self.pkg.name(self.db) => {
                self.package_ir(package).into_iter().collect()
            }
            _ => Vec::new(),
        }
    }

    /// The rules of the declaration the canonical type reference `reference`
    /// names; a bare reference is in the cursor's package.
    fn named_rules(&self, reference: &str) -> Vec<Rule> {
        let package = match reference.rsplit_once('.') {
            Some((package, _)) => package.to_string(),
            None => self.pkg.name(self.db).clone(),
        };
        let name = reference.rsplit('.').next().unwrap_or(reference);
        let Some(ir) = self.package_ir(&package) else {
            return Vec::new();
        };
        match ir.decls.iter().find(|decl| decl.name == name) {
            Some(decl) => rules::rules(&ir, &[], ItemRef::Decl(decl)),
            None => Vec::new(),
        }
    }
}

/// The hover for a member name at its declaration: a struct field, an enum
/// value, an enumset bit, a union arm, or a command or query parameter. Each
/// is read from the lowered IR, so a field's ordinal counts reserved
/// tombstones exactly as codegen does. Returns `None` when the cursor is not
/// on such a name.
fn member_hover(scope: Scope<'_>, file: InputFile, offset: TextSize) -> Option<HoverInfo> {
    let source = nav::source_file(scope.db, file);
    let token = nav::identifier_at(source.syntax(), offset)?;
    let name_node = token.parent()?;
    if name_node.kind() != SyntaxKind::Name {
        return None;
    }
    let member = name_node.parent()?;
    let name = token.text();
    let ir = check_package(scope.db, scope.ws, scope.pkg, scope.std).ir;
    let markdown = match member.kind() {
        SyntaxKind::FieldDef => field_markdown(scope, &ir, &member, name)?,
        SyntaxKind::EnumValue | SyntaxKind::EnumSetBit => {
            enum_member_markdown(scope, &ir, &member, name)?
        }
        SyntaxKind::UnionArm => arm_markdown(scope, &ir, &member, name)?,
        SyntaxKind::Param => param_markdown(scope, &ir, &member, name)?,
        _ => return None,
    };
    Some(HoverInfo {
        markdown,
        range: name_node.text_range(),
    })
}

/// The hover for a doc link or an `@see` target in `file` (a file of `pkg`),
/// for a file whose own hover does not go through [`hover`] — an `.rsdl`
/// file, whose declarations are doc carriers too.
pub(crate) fn doc_link_hover_at(
    db: &dyn salsa::Database,
    ws: Workspace,
    std: Package,
    pkg: Package,
    file: InputFile,
    offset: TextSize,
) -> Option<HoverInfo> {
    doc_link_hover(Scope { db, ws, std, pkg }, file, offset)
}

/// The hover for a doc link or an `@see` target: the hover of its target at
/// the target's own declaration site — a declaration's, a member's or an
/// interaction's, whichever the link names — anchored to the link's span.
fn doc_link_hover(scope: Scope<'_>, file: InputFile, offset: TextSize) -> Option<HoverInfo> {
    let link = nav::resolve_doc_link_at(scope.db, scope.ws, scope.std, scope.pkg, file, offset)?;
    let (target_file, site) = nav::canonical_site(
        scope.db,
        scope.ws,
        scope.std,
        scope.pkg,
        &link.target.canonical(),
    )?;
    let owner = owner_package(
        scope.db,
        scope.ws,
        scope.std,
        scope.pkg,
        &link.target.symbol.package,
    )?;
    let target = hover(
        scope.db,
        scope.ws,
        scope.std,
        owner,
        target_file,
        site.start(),
    )?;
    Some(HoverInfo {
        markdown: target.markdown,
        range: link.candidate.source,
    })
}

/// The hover for a member used inside a `require` or `ensure` clause
/// (ridl §13), the only expressions of the built grammar: a bare name that
/// is a parameter of the enclosing interaction or another interaction of the
/// same shape, or the member of a `Gear.PARK` access whose head names an enum
/// or an enumset. Each shows the hover of the member at its declaration.
fn contract_use_hover(scope: Scope<'_>, file: InputFile, offset: TextSize) -> Option<HoverInfo> {
    let source = nav::source_file(scope.db, file);
    let token = nav::identifier_at(source.syntax(), offset)?;
    if token.kind() != SyntaxKind::Ident {
        return None;
    }
    let node = token.parent()?;
    if !node.ancestors().any(|a| a.kind() == SyntaxKind::Attribute) {
        return None;
    }
    let markdown = match node.kind() {
        SyntaxKind::PathExpr => path_use_markdown(scope, &node, token.text())?,
        SyntaxKind::MemberExpr => {
            let member = MemberExpr::cast(node)?;
            if member.member_token()? != token {
                return None;
            }
            enum_use_markdown(scope, &member)?
        }
        _ => return None,
    };
    Some(HoverInfo {
        markdown,
        range: token.text_range(),
    })
}

/// A bare name in a contract clause: a parameter of the enclosing
/// interaction, else an interaction of the same shape (a `require` may read
/// the shape's own signal, ridl §13).
fn path_use_markdown(scope: Scope<'_>, path: &SyntaxNode, name: &str) -> Option<String> {
    let ir = check_package(scope.db, scope.ws, scope.pkg, scope.std).ir;
    if let Some(markdown) = param_markdown(scope, &ir, path, name) {
        return Some(markdown);
    }
    let interaction = path.ancestors().find_map(InterfaceMember::cast)?;
    let (owner, shape) = enclosing_shape(&ir, interaction.syntax())?;
    let decl = shape.interactions.iter().find(|decl| {
        decl.name == name && !matches!(decl.kind, Some(v2::decl::Kind::ReservedSlot(_)))
    })?;
    Some(interaction_markdown(scope, &ir, &owner, decl))
}

/// The member of a `Gear.PARK` or `pkg.Gear.PARK` access in a contract
/// clause, when the access resolves, through the doc-link resolver, to a
/// value of an enum or a bit of an enumset.
fn enum_use_markdown(scope: Scope<'_>, member: &MemberExpr) -> Option<String> {
    let segments = expr_segments(&Expr::Member(member.clone()))?;
    let resolution = resolve_package(scope.db, scope.ws, scope.pkg, scope.std);
    let target = resolve_doc_link(
        scope.db,
        scope.ws,
        scope.std,
        scope.pkg,
        &resolution,
        &segments,
    )
    .ok()?;
    let value = target.member.as_deref()?;
    if !matches!(target.symbol.kind, SymbolKind::Enum | SymbolKind::EnumSet) {
        return None;
    }
    let ir = scope.package_ir(&target.symbol.package)?;
    enum_value_markdown(scope, &ir, &target.symbol.name, value)
}

/// The dotted segments of a path or member-access expression, or `None` for
/// any other expression.
fn expr_segments(expr: &Expr) -> Option<Vec<String>> {
    match expr {
        Expr::Path(path) => Some(vec![path.name_token()?.text().to_string()]),
        Expr::Member(member) => {
            let mut segments = expr_segments(&member.base()?)?;
            segments.push(member.member_token()?.text().to_string());
            Some(segments)
        }
        _ => None,
    }
}

/// The name of the declaration `member` sits directly in, when that
/// declaration is of kind `kind`. A tuple field is a `FieldDef` too, but its
/// parent is the tuple type, so it is not taken for a struct field.
fn parent_declaration(member: &SyntaxNode, kind: SyntaxKind) -> Option<String> {
    let parent = member.parent().filter(|parent| parent.kind() == kind)?;
    let name = parent
        .children()
        .find(|child| child.kind() == SyntaxKind::Name)?;
    name.children_with_tokens()
        .filter_map(|element| element.into_token())
        .find(|token| token.kind() == SyntaxKind::Ident)
        .map(|token| token.text().to_string())
}

/// A struct field: its type and ordinal, then its doc, the rules of its type,
/// and its tags.
fn field_markdown(
    scope: Scope<'_>,
    ir: &v2::Package,
    member: &SyntaxNode,
    name: &str,
) -> Option<String> {
    let owner = parent_declaration(member, SyntaxKind::StructDef)?;
    let decl = ir.decls.iter().find(|decl| decl.name == owner)?;
    let Some(v2::decl::Kind::StructDef(struct_def)) = &decl.kind else {
        return None;
    };
    let field = struct_def
        .members
        .iter()
        .find_map(|slot| match &slot.member {
            Some(v2::struct_member::Member::Field(field)) if field.name == name => Some(field),
            _ => None,
        })?;
    let mut out = format!(
        "```typl\nfield {}.{owner}.{name} : {}\n```\n\n**Ordinal:** `#{}`",
        ir.name,
        type_text(field.r#type.as_ref()),
        field.ordinal,
    );
    let deps = scope.deps_of(field.r#type.as_ref());
    let deps: Vec<&v2::Package> = deps.iter().collect();
    let rules = rules::rules(ir, &deps, ItemRef::Field(field));
    let parts = DocParts {
        doc: &field.doc,
        links: &field.links,
        since: &field.since,
        labels: &field.labels,
        deprecated: field.deprecated.as_deref(),
    };
    push_doc_sections(&mut out, &parts, &rules, &|target| scope.resolve(target));
    Some(out)
}

/// An enum value or an enumset bit: its number, then its doc and `@since`.
fn enum_member_markdown(
    scope: Scope<'_>,
    ir: &v2::Package,
    member: &SyntaxNode,
    name: &str,
) -> Option<String> {
    let owner_kind = if member.kind() == SyntaxKind::EnumValue {
        SyntaxKind::EnumDef
    } else {
        SyntaxKind::EnumSetDef
    };
    let owner = parent_declaration(member, owner_kind)?;
    enum_value_markdown(scope, ir, &owner, name)
}

/// The value or bit `name` of the enum or enumset `owner` of `ir`: its
/// number, then its doc and `@since`.
fn enum_value_markdown(
    scope: Scope<'_>,
    ir: &v2::Package,
    owner: &str,
    name: &str,
) -> Option<String> {
    let decl = ir.decls.iter().find(|decl| decl.name == owner)?;
    let values = match &decl.kind {
        Some(v2::decl::Kind::EnumDef(enum_def)) => &enum_def.values,
        Some(v2::decl::Kind::EnumSetDef(set)) => &set.bits,
        _ => return None,
    };
    let value = values.iter().find(|value| value.name == name)?;
    let mut out = format!("```typl\n{}.{owner}.{name} = {}\n```", ir.name, value.value);
    let parts = DocParts {
        doc: &value.doc,
        links: &value.links,
        since: &value.since,
        ..DocParts::default()
    };
    push_doc_sections(&mut out, &parts, &[], &|target| scope.resolve(target));
    Some(out)
}

/// A union arm: its type and ordinal, then its doc, the rules of its type,
/// and `@since`.
fn arm_markdown(
    scope: Scope<'_>,
    ir: &v2::Package,
    member: &SyntaxNode,
    name: &str,
) -> Option<String> {
    let owner = parent_declaration(member, SyntaxKind::UnionDef)?;
    let decl = ir.decls.iter().find(|decl| decl.name == owner)?;
    let Some(v2::decl::Kind::UnionDef(union_def)) = &decl.kind else {
        return None;
    };
    let arm = union_def.arms.iter().find(|arm| arm.name == name)?;
    let mut out = format!(
        "```typl\narm {}.{owner}.{name} : {}\n```\n\n**Ordinal:** `#{}`",
        ir.name, arm.type_ref, arm.ordinal,
    );
    let rules = scope.named_rules(&arm.type_ref);
    let parts = DocParts {
        doc: &arm.doc,
        links: &arm.links,
        since: &arm.since,
        ..DocParts::default()
    };
    push_doc_sections(&mut out, &parts, &rules, &|target| scope.resolve(target));
    Some(out)
}

/// A command or query parameter: its type, then its doc — or, when it has
/// none, the doc of its named type under the line "From `TypeName`:" — the
/// rules of its type, and `@since`.
fn param_markdown(
    scope: Scope<'_>,
    ir: &v2::Package,
    member: &SyntaxNode,
    name: &str,
) -> Option<String> {
    let interaction = member.ancestors().find_map(InterfaceMember::cast)?;
    let interaction_name = interaction.name()?.ident_token()?.text().to_string();
    let (owner, shape) = enclosing_shape(ir, interaction.syntax())?;
    let decl = shape.interactions.iter().find(|decl| {
        decl.name == interaction_name && !matches!(decl.kind, Some(v2::decl::Kind::ReservedSlot(_)))
    })?;
    let params = match &decl.kind {
        Some(v2::decl::Kind::CommandDef(command)) => &command.params,
        Some(v2::decl::Kind::QueryDef(query)) => &query.params,
        _ => return None,
    };
    let param = params.iter().find(|param| param.name == name)?;
    let mut out = format!(
        "```ridl\nparam {owner}.{interaction_name}.{name} : {}\n```",
        type_text(param.r#type.as_ref()),
    );
    let deps = scope.deps_of(param.r#type.as_ref());
    let deps: Vec<&v2::Package> = deps.iter().collect();
    let rules = rules::rules(ir, &deps, ItemRef::Param(param));
    let resolve = |target: &str| scope.resolve(target);

    let fallback = if param.doc.trim().is_empty() {
        param
            .r#type
            .as_ref()
            .and_then(named_ref)
            .and_then(|named| decl_of_ref(scope.db, scope.ws, scope.std, scope.pkg, named))
            .filter(|type_decl| !type_decl.doc.trim().is_empty())
    } else {
        None
    };
    let parts = match &fallback {
        Some(type_decl) => {
            out.push_str(&format!(
                "\n\nFrom `{}`:\n\n{}",
                type_decl.name,
                render_doc(&type_decl.doc, &type_decl.links, resolve),
            ));
            DocParts {
                since: &param.since,
                ..DocParts::default()
            }
        }
        None => DocParts {
            doc: &param.doc,
            links: &param.links,
            since: &param.since,
            ..DocParts::default()
        },
    };
    push_doc_sections(&mut out, &parts, &rules, &resolve);
    Some(out)
}

/// The source form of an optional field type, `?` when it is absent.
fn type_text(field_type: Option<&v2::FieldType>) -> String {
    field_type
        .map(field_type_text)
        .unwrap_or_else(|| "?".to_string())
}

/// The 1-based ordinal of field `field_name` in struct `struct_name`, from the
/// lowered IR. Shared with the inlay-hint pass, which renders the same
/// ordinal beside every field.
pub(crate) fn field_ordinal(ir: &v2::Package, struct_name: &str, field_name: &str) -> Option<u32> {
    let decl = ir.decls.iter().find(|decl| decl.name == struct_name)?;
    let Some(v2::decl::Kind::StructDef(struct_def)) = &decl.kind else {
        return None;
    };
    struct_def
        .members
        .iter()
        .find_map(|member| match &member.member {
            Some(v2::struct_member::Member::Field(field)) if field.name == field_name => {
                Some(field.ordinal)
            }
            _ => None,
        })
}

/// The hover markdown for a resolved symbol: the declaration's IR rendering when
/// the symbol lowered, or a minimal name-and-kind line as a fallback.
///
/// `pkg` is the package the cursor was in; it is preferred when its name matches
/// so a symbol declared in a standalone overlay (which `package_of` cannot find)
/// still renders its full IR — mirroring the checker's own `package_handle`.
pub(crate) fn symbol_markdown(
    db: &dyn salsa::Database,
    ws: Workspace,
    std: Package,
    pkg: Package,
    symbol: &Symbol,
) -> String {
    let qualified = format!("{}.{}", symbol.package, symbol.name);
    let resolve = |target: &str| link_location(db, ws, std, pkg, target);
    if let Some(target) = owner_package(db, ws, std, pkg, &symbol.package) {
        let ir = check_package(db, ws, target, std).ir;
        if let Some(decl) = ir.decls.iter().find(|decl| decl.name == symbol.name) {
            let rules = rules::rules(&ir, &[], ItemRef::Decl(decl));
            return render_decl(&qualified, decl, &rules, &resolve);
        }
        if symbol.kind == SymbolKind::Interface
            && let Some(interface) = named_interface(&ir, &symbol.name)
        {
            let mut out = format!("```ridl\ninterface {qualified}\n```");
            push_doc_sections(&mut out, &interface_parts(interface), &[], &resolve);
            return out;
        }
    }
    format!("**`{qualified}`** — {}", symbol_kind(symbol.kind))
}

/// The docs of resolved symbols for completion items (ADR-0026): the
/// rendered doc, `@since`, the labels and the deprecation reason, without
/// the signature or the Contract list. The IR of each owning package is read
/// once per completion request, not once per item.
pub(crate) struct SymbolDocs<'db> {
    scope: Scope<'db>,
    irs: HashMap<String, Option<v2::Package>>,
}

impl<'db> SymbolDocs<'db> {
    /// The docs as seen from `pkg`, the package of the file being completed.
    pub(crate) fn new(
        db: &'db dyn salsa::Database,
        ws: Workspace,
        std: Package,
        pkg: Package,
    ) -> Self {
        Self {
            scope: Scope { db, ws, std, pkg },
            irs: HashMap::new(),
        }
    }

    /// The doc of `symbol`, or `None` when it has none.
    pub(crate) fn get(&mut self, symbol: &Symbol) -> Option<String> {
        let scope = self.scope;
        let ir = self
            .irs
            .entry(symbol.package.clone())
            .or_insert_with(|| scope.package_ir(&symbol.package))
            .as_ref()?;
        let parts = match ir.decls.iter().find(|decl| decl.name == symbol.name) {
            Some(decl) => decl_parts(decl),
            None => interface_parts(named_interface(ir, &symbol.name)?),
        };
        let mut out = String::new();
        push_doc_sections(&mut out, &parts, &[], &|target| scope.resolve(target));
        let out = out.trim_start();
        (!out.is_empty()).then(|| out.to_string())
    }
}

/// The `interface` declaration `name` of `ir`. The walk is over every
/// shape; an inline shape is keyed by its dotted service name, which no
/// interface name spells.
fn named_interface<'a>(ir: &'a v2::Package, name: &str) -> Option<&'a v2::Interface> {
    ir.shapes()
        .find(|shape| shape.name == name)
        .map(|shape| shape.interface)
}

/// The package named `name`: the cursor's package `pkg` when the names match
/// (a standalone overlay, which `package_of` cannot find, included), the
/// embedded `ridl.std`, or a workspace package.
fn owner_package(
    db: &dyn salsa::Database,
    ws: Workspace,
    std: Package,
    pkg: Package,
    name: &str,
) -> Option<Package> {
    if name == pkg.name(db) {
        Some(pkg)
    } else if name == std.name(db) {
        Some(std)
    } else {
        package_of(db, ws, name.to_string())
    }
}

/// The location of the canonical doc-link target `target` (`pkg.Name` or
/// `pkg.Name.member`): the target's file and the range of its name. `None`
/// when the target is not found or its file has no `file://` URI.
pub(crate) fn link_location(
    db: &dyn salsa::Database,
    ws: Workspace,
    std: Package,
    pkg: Package,
    target: &str,
) -> Option<lt::Location> {
    let (file, range) = nav::canonical_site(db, ws, std, pkg, target)?;
    let uri = convert::path_to_uri(file.path(db))?;
    Some(lt::Location {
        uri,
        range: convert::line_index(file.text(db)).range(range),
    })
}

/// Renders a doc body as Markdown: each resolved doc link becomes a link to
/// its target's location, a `file:` URI with a 1-based line fragment
/// (`#L12`). A link `resolve` gives no location for becomes a code span.
///
/// `links` carry byte offsets into `doc` (ADR-0026). A link whose range does
/// not fall on character boundaries of `doc`, or overlaps an earlier link, is
/// left as written. A `@see` entry has length 0 and is skipped.
pub fn render_doc(
    doc: &str,
    links: &[v2::DocLink],
    resolve: impl Fn(&str) -> Option<lt::Location>,
) -> String {
    let mut links: Vec<&v2::DocLink> = links.iter().filter(|link| link.len > 0).collect();
    links.sort_by_key(|link| link.offset);
    let mut out = String::with_capacity(doc.len());
    let mut written = 0;
    for link in links {
        let start = link.offset as usize;
        let end = start + link.len as usize;
        if start < written || doc.get(start..end).is_none() {
            continue;
        }
        out.push_str(&doc[written..start]);
        match resolve(&link.target) {
            Some(location) => out.push_str(&format!(
                "[{}]({}#L{})",
                link.text,
                location.uri.as_str(),
                location.range.start.line + 1
            )),
            None if link.text.starts_with('`') => out.push_str(&link.text),
            None => out.push_str(&format!("`{}`", link.text)),
        }
        written = end;
    }
    out.push_str(&doc[written..]);
    out
}

/// The doc fields of one carrier, borrowed from its IR message. A carrier
/// without labels or a deprecation leaves those fields empty.
#[derive(Default)]
pub(crate) struct DocParts<'a> {
    pub doc: &'a str,
    pub links: &'a [v2::DocLink],
    pub since: &'a [String],
    pub labels: &'a [String],
    pub deprecated: Option<&'a str>,
}

fn decl_parts(decl: &v2::Decl) -> DocParts<'_> {
    DocParts {
        doc: &decl.doc,
        links: &decl.links,
        since: &decl.since,
        labels: &decl.labels,
        deprecated: decl.deprecated.as_deref(),
    }
}

fn interface_parts(interface: &v2::Interface) -> DocParts<'_> {
    DocParts {
        doc: &interface.doc,
        links: &interface.links,
        since: &interface.since,
        labels: &interface.labels,
        deprecated: interface.deprecated.as_deref(),
    }
}

/// Appends what follows the signature in a hover (ADR-0026), in order: the
/// rendered doc, a **Contract** list with one item per rule (left out when no
/// rule renders to text), then `@since`, the labels and the deprecation
/// reason.
pub(crate) fn push_doc_sections(
    out: &mut String,
    parts: &DocParts<'_>,
    rules: &[Rule],
    resolve: &dyn Fn(&str) -> Option<lt::Location>,
) {
    if !parts.doc.trim().is_empty() {
        out.push_str("\n\n");
        out.push_str(&render_doc(parts.doc, parts.links, resolve));
    }
    let items: Vec<String> = rules
        .iter()
        .map(rules::render)
        .filter(|item| !item.is_empty())
        .collect();
    if !items.is_empty() {
        out.push_str("\n\n**Contract**\n");
        for item in items {
            out.push_str(&format!("\n- {item}"));
        }
    }
    if !parts.since.is_empty() {
        out.push_str(&format!("\n\n**Since:** {}", parts.since.join(", ")));
    }
    if !parts.labels.is_empty() {
        out.push_str(&format!("\n\n**Labels:** {}", parts.labels.join(", ")));
    }
    if let Some(reason) = parts.deprecated {
        out.push_str(&format!("\n\n**Deprecated:** {reason}"));
    }
}

/// Renders one IR declaration as a hover markdown block: a fenced typl
/// signature line and the derived width, then the doc sections of
/// [`push_doc_sections`].
fn render_decl(
    qualified: &str,
    decl: &v2::Decl,
    rules: &[Rule],
    resolve: &dyn Fn(&str) -> Option<lt::Location>,
) -> String {
    let mut lines = String::new();
    lines.push_str("```typl\n");
    lines.push_str(&signature(qualified, decl));
    lines.push_str("\n```");

    if let Some(v2::decl::Kind::TypeDef(type_def)) = &decl.kind
        && let Some(width) = type_def.width.as_ref().map(width_name)
    {
        lines.push_str(&format!("\n\n**Width:** `{width}`"));
    }
    push_doc_sections(&mut lines, &decl_parts(decl), rules, resolve);
    lines
}

/// The one-line typl signature of a declaration.
fn signature(qualified: &str, decl: &v2::Decl) -> String {
    let modifiers = declaration_modifiers(decl);
    match &decl.kind {
        Some(v2::decl::Kind::TypeDef(type_def)) => {
            format!(
                "{modifiers}type {qualified}{}{}{}",
                backing(type_def.backing.as_ref()),
                constraint(type_def.constraint.as_ref()),
                init(type_def.init.as_ref(), type_def.declared_init.as_ref()),
            )
        }
        Some(v2::decl::Kind::ConstDef(const_def)) => {
            if let Some(regex) = &const_def.regex {
                format!("{modifiers}const {qualified} = {regex}")
            } else {
                let type_ref = const_def
                    .type_ref
                    .as_deref()
                    .map(|name| format!(" : {name}"))
                    .unwrap_or_default();
                format!(
                    "{modifiers}const {qualified}{type_ref} = {}",
                    const_def.value
                )
            }
        }
        Some(v2::decl::Kind::StructDef(_)) => format!("{modifiers}struct {qualified}"),
        Some(v2::decl::Kind::EnumDef(_)) => format!("{modifiers}enum {qualified}"),
        Some(v2::decl::Kind::EnumSetDef(_)) => format!("{modifiers}enumset {qualified}"),
        Some(v2::decl::Kind::UnionDef(_)) => format!("{modifiers}union {qualified}"),
        // Interaction kinds ride `Interface.interactions`, never a package
        // decl; interaction hovers land with the E2 LSP tasks.
        Some(_) | None => qualified.to_string(),
    }
}

/// The `internal` / `error` modifier prefix (with a trailing space) for a
/// declaration's signature.
fn declaration_modifiers(decl: &v2::Decl) -> String {
    let mut prefix = String::new();
    if decl.visibility == v2::Visibility::Internal as i32 {
        prefix.push_str("internal ");
    }
    if decl.is_error {
        prefix.push_str("error ");
    }
    prefix
}

/// The backing clause of a type (`: km/h`, `: integer`), or the empty string
/// when the backing is missing.
fn backing(backing: Option<&v2::Backing>) -> String {
    match backing.and_then(|backing| backing.kind.as_ref()) {
        Some(v2::backing::Kind::Unit(unit)) => format!(" : {unit}"),
        Some(v2::backing::Kind::Primitive(primitive)) => {
            format!(" : {}", primitive_name(*primitive))
        }
        None => String::new(),
    }
}

/// The constraint clause of a type (`[0.0..250.0 step 0.5]`, `[0..256]`,
/// `[..100]`, `match /.../`), or the empty string when there is no constraint.
fn constraint(constraint: Option<&v2::Constraint>) -> String {
    let Some(constraint) = constraint else {
        return String::new();
    };
    // An open-ended range lowers with one bound absent (`[..100]`, `[0..]`);
    // render the present side and leave the other empty (ADR-0004 §10).
    if constraint.min.is_some() || constraint.max.is_some() {
        let min = constraint.min.as_deref().unwrap_or("");
        let max = constraint.max.as_deref().unwrap_or("");
        let step = constraint
            .step
            .as_deref()
            .map(|step| format!(" step {step}"))
            .unwrap_or_default();
        return format!(" [{min}..{max}{step}]");
    }
    if constraint.len_min.is_some() || constraint.len_max.is_some() {
        let min = constraint.len_min.unwrap_or(0);
        let max = constraint.len_max.unwrap_or(0);
        return format!(" [{min}..{max}]");
    }
    if let Some(pattern) = &constraint.pattern {
        return format!(" match {pattern}");
    }
    if let Some(pattern_const) = &constraint.pattern_const {
        return format!(" match {pattern_const}");
    }
    String::new()
}

/// The init clause of a type (`= 0.0`): the declared init when present,
/// otherwise the resolved derived value, otherwise the empty string.
fn init(init: Option<&v2::InitValue>, declared: Option<&String>) -> String {
    if let Some(declared) = declared {
        return format!(" = {declared}");
    }
    match init.and_then(|init| init.value.as_ref()) {
        Some(value) => format!(" = {value}"),
        None => String::new(),
    }
}

/// The display name of a derived wire width.
fn width_name(width: &v2::type_def::Width) -> &'static str {
    match width {
        v2::type_def::Width::IntWidth(int_width) => int_width_name(*int_width),
        v2::type_def::Width::FloatWidth(float_width) => float_width_name(*float_width),
    }
}

/// The lowercase display name of a primitive type.
fn primitive_name(primitive: i32) -> &'static str {
    match v2::PrimitiveType::try_from(primitive) {
        Ok(v2::PrimitiveType::Boolean) => "boolean",
        Ok(v2::PrimitiveType::Integer) => "integer",
        Ok(v2::PrimitiveType::Float) => "float",
        Ok(v2::PrimitiveType::String) => "string",
        Ok(v2::PrimitiveType::Bytes) => "bytes",
        _ => "?",
    }
}

/// The lowercase display name of an integer wire width.
fn int_width_name(width: i32) -> &'static str {
    match v2::IntWidth::try_from(width) {
        Ok(v2::IntWidth::U8) => "u8",
        Ok(v2::IntWidth::I8) => "i8",
        Ok(v2::IntWidth::U16) => "u16",
        Ok(v2::IntWidth::I16) => "i16",
        Ok(v2::IntWidth::U32) => "u32",
        Ok(v2::IntWidth::I32) => "i32",
        Ok(v2::IntWidth::U64) => "u64",
        Ok(v2::IntWidth::I64) => "i64",
        _ => "?",
    }
}

/// The lowercase display name of a float wire width.
fn float_width_name(width: i32) -> &'static str {
    match v2::FloatWidth::try_from(width) {
        Ok(v2::FloatWidth::F32) => "f32",
        Ok(v2::FloatWidth::F64) => "f64",
        _ => "?",
    }
}

/// The one-word kind label of a resolver symbol — the fallback used when the
/// symbol did not lower to IR.
fn symbol_kind(kind: SymbolKind) -> &'static str {
    match kind {
        SymbolKind::Type => "type",
        SymbolKind::Const => "const",
        SymbolKind::Struct => "struct",
        SymbolKind::Enum => "enum",
        SymbolKind::EnumSet => "enumset",
        SymbolKind::Union => "union",
        SymbolKind::Interface => "interface",
    }
}

// --- the ridl interaction layer ---------------------------------

/// The general form §6.4 wording for Stratum 3. Stratum 3 is fully detected by
/// the runtime — acks, timeouts, staleness — and merely absent from the
/// contract language; calling it undefined behavior would say the opposite of
/// what the design guarantees, so this sentence is normative and quoted
/// verbatim.
const STRATUM_THREE: &str = "infrastructure failure — detected, undeclared";

/// The note every service hover closes with: the ridl §14.5 posture
/// neutrality, and rsdl v0.2's reservation of the posture (rsdl §12).
const POSTURE_NOTE: &str = "Posture-neutral by design: this declaration says nothing about how the \
    contract is realized on the wire — static (its signals and events packed into bus frames) or \
    discovered (SOME/IP, DDS, uProtocol) (ridl §14.5). rsdl derives no posture in this release: \
    deriving the posture per deployment is reserved (rsdl §12).";

/// The hover for an interaction: the cursor on an interaction's name, or on the
/// `|` of its inline fallible return.
///
/// The rendering is driven entirely by the interaction's lowered `Decl`, found
/// by name inside the enclosing shape's IR, so the ordinal and the timing are
/// the ones codegen and `ridl diff` see — never re-derived from the tree.
fn interaction_hover(
    db: &dyn salsa::Database,
    ws: Workspace,
    std: Package,
    pkg: Package,
    file: InputFile,
    offset: TextSize,
) -> Option<HoverInfo> {
    let source = nav::source_file(db, file);
    let token = nav::identifier_at(source.syntax(), offset)?;
    let anchor = token.parent()?;
    // Two anchors reach the same rendering: the interaction's own name, and
    // the `T | E` of a fallible return (whose arms are part of that rendering).
    let (member, range) = match anchor.kind() {
        SyntaxKind::Name => {
            let member = InterfaceMember::cast(anchor.parent()?)?;
            (member, anchor.text_range())
        }
        SyntaxKind::FallibleType => {
            let member = anchor.ancestors().find_map(InterfaceMember::cast)?;
            (member, anchor.text_range())
        }
        _ => return None,
    };
    // A tombstone declares nothing to render; its ordinal rides the inlay hint.
    if matches!(member, InterfaceMember::Reserved(_)) {
        return None;
    }
    let name = member.name()?.ident_token()?.text().to_string();

    let ir = &check_package(db, ws, pkg, std).ir;
    let (owner, shape) = enclosing_shape(ir, member.syntax())?;
    let decl = shape.interactions.iter().find(|decl| {
        decl.name == name && !matches!(decl.kind, Some(v2::decl::Kind::ReservedSlot(_)))
    })?;

    let markdown = interaction_markdown(Scope { db, ws, std, pkg }, ir, &owner, decl);
    Some(HoverInfo { markdown, range })
}

/// The whole hover of an interaction of `ir`: the head of
/// [`render_interaction`], then the doc sections with the interaction's rules.
fn interaction_markdown(
    scope: Scope<'_>,
    ir: &v2::Package,
    owner: &str,
    decl: &v2::Decl,
) -> String {
    let Scope { db, ws, std, pkg } = scope;
    let mut markdown = render_interaction(db, ws, std, pkg, owner, decl);
    let rules = rules::rules(ir, &[], ItemRef::Decl(decl));
    push_doc_sections(&mut markdown, &decl_parts(decl), &rules, &|target| {
        scope.resolve(target)
    });
    markdown
}

/// The hover for a service declaration: the cursor on its dotted global name.
fn service_hover(
    db: &dyn salsa::Database,
    ws: Workspace,
    pkg: Package,
    std: Package,
    file: InputFile,
    offset: TextSize,
) -> Option<HoverInfo> {
    let source = nav::source_file(db, file);
    let token = nav::identifier_at(source.syntax(), offset)?;
    let dotted = token.parent()?;
    if dotted.kind() != SyntaxKind::DottedName {
        return None;
    }
    let service_node = dotted.parent()?;
    if service_node.kind() != SyntaxKind::ServiceDef {
        return None;
    }
    let name = non_empty(ServiceDef::cast(service_node)?.name()?.text())?;

    let ir = &check_package(db, ws, pkg, std).ir;
    let service = ir.services.iter().find(|service| service.name == name)?;
    Some(HoverInfo {
        markdown: render_service(service, &|target| link_location(db, ws, std, pkg, target)),
        range: dotted.text_range(),
    })
}

/// A dotted name's text, or `None` when the parser recovered the declaration
/// with no tokens at all. The text itself is [`DottedName::text`] — the key
/// `v2::Service.name` carries, so it is what the IR lookup matches on.
fn non_empty(text: String) -> Option<String> {
    (!text.is_empty()).then_some(text)
}

/// The interface shape a node sits inside, with the name to display for it: an
/// `interface` declaration's own name, or the dotted name of the service whose
/// inline shape holds the node (ridl §14.5). `None` when the enclosing shape
/// did not lower — a file that failed to parse past the header, most often.
fn enclosing_shape<'a>(
    ir: &'a v2::Package,
    node: &SyntaxNode,
) -> Option<(String, &'a v2::Interface)> {
    for ancestor in node.ancestors() {
        let name = match ancestor.kind() {
            SyntaxKind::InterfaceDef => InterfaceDef::cast(ancestor)?
                .name()?
                .ident_token()?
                .text()
                .to_string(),
            SyntaxKind::ServiceDef => non_empty(ServiceDef::cast(ancestor)?.name()?.text())?,
            _ => continue,
        };
        // One lookup for both: `Package::shapes` keys a named interface on its
        // own name and a service's inline shape on the dotted service name, so
        // a shape stored outside `Package.interfaces` is still found.
        let shape = ir.shapes().find(|shape| shape.name == name)?;
        return Some((name, shape.interface));
    }
    None
}

/// Renders the head of one interaction's hover: the signature, the §11
/// ordinal, the payload with its typl detail, the resolved timing — on an
/// RPC, the declared bound or the default one the checker filled in — with its
/// per-kind reading, and the error strata note for a fallible return. The caller appends the doc sections.
fn render_interaction(
    db: &dyn salsa::Database,
    ws: Workspace,
    std: Package,
    pkg: Package,
    owner: &str,
    decl: &v2::Decl,
) -> String {
    let mut out = String::new();
    out.push_str("```ridl\n");
    out.push_str(&interaction_signature(owner, decl));
    out.push_str("\n```");
    out.push_str(&format!(
        "\n\n**Ordinal:** `#{}` — declaration order is wire identity (ridl §11)",
        decl.ordinal
    ));

    match &decl.kind {
        Some(v2::decl::Kind::SignalDef(signal)) => {
            out.push_str(&payload_line(db, ws, std, pkg, &signal.payload));
            if let Some(timing) = &signal.timing {
                out.push_str(&timing_line(timing, Reading::State));
            }
        }
        Some(v2::decl::Kind::EventDef(event)) => {
            out.push_str(&payload_line(db, ws, std, pkg, &event.payload));
            if let Some(timing) = &event.timing {
                out.push_str(&timing_line(timing, Reading::Occurrence));
            }
        }
        Some(v2::decl::Kind::FixedDef(fixed_def)) => {
            if let Some(named) = fixed_def.payload.as_ref().and_then(named_ref) {
                out.push_str(&payload_line(db, ws, std, pkg, named));
            }
        }
        Some(v2::decl::Kind::CommandDef(command)) => {
            // The checker resolves every command and query to a bound, the
            // default one when none is written (ridl §9.3); an absent timing
            // renders nothing. The query arm below is the same.
            if let Some(timing) = &command.timing {
                out.push_str(&timing_line(timing, Reading::Acceptance));
            }
        }
        Some(v2::decl::Kind::QueryDef(query)) => {
            if let Some(fallible) = query.return_type.as_ref().and_then(fallible_of) {
                out.push_str(&strata_note(fallible));
            }
            if let Some(timing) = &query.timing {
                out.push_str(&timing_line(timing, Reading::Reply));
            }
        }
        _ => {}
    }
    out
}

/// The one-line ridl signature of an interaction, with every reference in the
/// canonical form the IR stores — never an import alias (ridl §14.1).
fn interaction_signature(owner: &str, decl: &v2::Decl) -> String {
    let name = format!("{owner}.{}", decl.name);
    match &decl.kind {
        Some(v2::decl::Kind::SignalDef(signal)) => format!(
            "signal {name} : {}{}{}",
            signal.payload,
            signal
                .declared_init
                .as_ref()
                .map(|init| format!(" = {init}"))
                .unwrap_or_default(),
            timing_suffix(signal.timing.as_ref()),
        ),
        Some(v2::decl::Kind::EventDef(event)) => format!(
            "event {name} : {}{}",
            event.payload,
            timing_suffix(event.timing.as_ref()),
        ),
        Some(v2::decl::Kind::CommandDef(command)) => format!(
            "command {name}({}){}",
            params(&command.params),
            timing_suffix(command.timing.as_ref()),
        ),
        Some(v2::decl::Kind::QueryDef(query)) => format!(
            "query {name}({}): {}{}",
            params(&query.params),
            query
                .return_type
                .as_ref()
                .map(return_text)
                .unwrap_or_else(|| "?".to_string()),
            timing_suffix(query.timing.as_ref()),
        ),
        Some(v2::decl::Kind::FixedDef(fixed_def)) => format!(
            "fixed {name} : {}",
            fixed_def
                .payload
                .as_ref()
                .map(field_type_text)
                .unwrap_or_else(|| "?".to_string()),
        ),
        _ => name,
    }
}

/// The `**Payload:**` line for a named payload reference, with the referenced
/// declaration's own typl detail — its unit and constraint for a scalar type,
/// its shape word for a composite. A payload that does not resolve contributes
/// the reference alone rather than a wrong reading.
fn payload_line(
    db: &dyn salsa::Database,
    ws: Workspace,
    std: Package,
    pkg: Package,
    canonical: &str,
) -> String {
    match decl_of_ref(db, ws, std, pkg, canonical)
        .as_ref()
        .and_then(type_summary)
    {
        Some(summary) => format!("\n\n**Payload:** `{canonical}` — {summary}"),
        None => format!("\n\n**Payload:** `{canonical}`"),
    }
}

/// Which per-kind reading general form §6.2 derives for an interaction: a
/// signal carries state, an event carries occurrences, and an RPC responds —
/// a command with its acceptance, a query with its reply (ridl §9.3).
#[derive(Clone, Copy)]
enum Reading {
    State,
    Occurrence,
    Acceptance,
    Reply,
}

impl Reading {
    /// The derived consequence of the generic bounds. The annotation itself
    /// means one thing everywhere — `min` is a rate floor, `max` a staleness
    /// bound — and only the declaring keyword decides what happens at each
    /// edge (general form §6.2). On an RPC the derived pair is the call
    /// throttle and the response bound, and what responding means is itself
    /// per kind: for a command it is acceptance — the §6.1 acknowledgment,
    /// not execution — and for a query it is the reply (ADR-0015 decision 3).
    fn text(self) -> &'static str {
        match self {
            Self::State => "min = rate floor (debounce), max = staleness bound (refresh ceiling)",
            Self::Occurrence => {
                "min = rate floor (throttle), max = staleness bound \
                 (TTL: stale occurrences discarded)"
            }
            Self::Acceptance => {
                "min = call throttle (the caller must not call faster), \
                 max = response bound (acceptance: the §6.1 acknowledgment, not execution)"
            }
            Self::Reply => {
                "min = call throttle (the caller must not call faster), \
                 max = response bound (the reply)"
            }
        }
    }
}

/// The `**Timing:**` line: the resolved mode, the resolved bounds, a note on
/// what the configured default supplied, and the derived per-kind reading.
///
/// On a signal or event `default_applied` means the whole default was
/// applied. On a command or query it means at least the maximum came from the
/// default: an untimed member takes the whole default, and `@[min..]` keeps
/// its written minimum. The IR does not record which of the two it was, so an
/// RPC with a minimum names only the maximum as defaulted, which is true in
/// both cases.
fn timing_line(timing: &v2::Timing, reading: Reading) -> String {
    let mode = match v2::TimingMode::try_from(timing.mode) {
        Ok(v2::TimingMode::StrictPeriodic) => "strict periodic",
        _ => "range",
    };
    let bounds = bounds_text(timing);
    let rpc = matches!(reading, Reading::Acceptance | Reading::Reply);
    let default = match (timing.default_applied, timing.min_us.as_deref()) {
        (false, _) => String::new(),
        (true, Some(_)) if rpc => {
            let max = timing.max_us.as_deref();
            let max = max
                .map(|value| duration_text(value, common_unit(&[Some(value)])))
                .unwrap_or_default();
            format!(" (maximum {max} taken from the default)")
        }
        (true, _) => format!(" (default {bounds} applied)"),
    };
    format!(
        "\n\n**Timing:** {mode} `{bounds}`{default} — {}",
        reading.text()
    )
}

/// The `@…` suffix of an interaction signature, for the resolved timing.
fn timing_suffix(timing: Option<&v2::Timing>) -> String {
    timing
        .map(|timing| format!(" @{}", bounds_text(timing)))
        .unwrap_or_default()
}

/// The duration units of ridl §2.1, coarsest first, with their microsecond
/// scale.
const DURATION_UNITS: &[(u128, &str)] = &[
    (3_600_000_000, "h"),
    (60_000_000, "min"),
    (1_000_000, "s"),
    (1_000, "ms"),
];

/// The resolved bounds in source form: the single period of a strict-periodic
/// annotation, or the `[min..max]` range with an absent side left empty.
///
/// Both bounds of a range render in one unit, so `[100ms..1000ms]` stays
/// readable as a ratio instead of becoming `[100ms..1s]`, which would make the
/// reader do the arithmetic to compare the two ends.
fn bounds_text(timing: &v2::Timing) -> String {
    let min = timing.min_us.as_deref();
    let max = timing.max_us.as_deref();
    let unit = common_unit(&[min, max]);
    if v2::TimingMode::try_from(timing.mode) == Ok(v2::TimingMode::StrictPeriodic) {
        // Strict periodic stores the one period in both bounds.
        return min
            .or(max)
            .map(|value| duration_text(value, unit))
            .unwrap_or_else(|| "?".to_string());
    }
    format!(
        "[{}..{}]",
        min.map(|value| duration_text(value, unit))
            .unwrap_or_default(),
        max.map(|value| duration_text(value, unit))
            .unwrap_or_default(),
    )
}

/// The coarsest duration unit that renders every present bound as a whole
/// number, or microseconds when no coarser unit divides them all. The IR's
/// exactness rule reaches the hover text: a bound is never rounded to make a
/// nicer unit fit.
fn common_unit(bounds: &[Option<&str>]) -> (u128, &'static str) {
    let values: Vec<u128> = bounds
        .iter()
        .flatten()
        .filter_map(|value| value.parse::<u128>().ok())
        .collect();
    if values.len() != bounds.iter().flatten().count() || values.is_empty() {
        return (1, "us");
    }
    DURATION_UNITS
        .iter()
        .copied()
        .find(|(scale, _)| {
            values
                .iter()
                .all(|value| *value >= *scale && value % scale == 0)
        })
        .unwrap_or((1, "us"))
}

/// Renders one exact-decimal microsecond bound in `unit`. A value the unit
/// cannot express exactly — a fractional microsecond count, which ridl §2.1
/// already flags at the source — falls back to raw microseconds.
fn duration_text(microseconds: &str, (scale, unit): (u128, &'static str)) -> String {
    match microseconds.parse::<u128>() {
        Ok(value) if value % scale == 0 => format!("{}{unit}", value / scale),
        _ => format!("{microseconds}us"),
    }
}

/// The ridl §10 error-strata note for an inline fallible return, closing with
/// the general form §6.4 wording for Stratum 3.
fn strata_note(fallible: &v2::FallibleType) -> String {
    format!(
        "\n\n**Returns:** `{ok}` on success, `{err}` on failure\
         \n\n**Errors (ridl §10):** Stratum 1 — the declared `{err}` arm is functional failure, \
         carried as data. Stratum 2 — a contract violation is derived from `require`/`ensure`, \
         never an error type. Stratum 3 — {STRATUM_THREE}.",
        ok = fallible.ok,
        err = fallible.err,
    )
}

/// Renders a service declaration: its list of interfaces, the ridl §14.5
/// posture note, and the doc sections. A named-form service renders every
/// reference in source order (ADR-0015 decision 12), so the hover shows the
/// same list the source declares.
pub(crate) fn render_service(
    service: &v2::Service,
    resolve: &dyn Fn(&str) -> Option<lt::Location>,
) -> String {
    let inline = service.shapes.iter().find_map(|slot| match &slot.kind {
        Some(v2::service_shape::Kind::Inline(shape)) => Some(shape),
        _ => None,
    });
    let slots: Vec<String> = service
        .shapes
        .iter()
        .filter_map(|slot| match &slot.kind {
            Some(v2::service_shape::Kind::InterfaceRef(interface)) => Some(interface.clone()),
            Some(v2::service_shape::Kind::Inline(_)) | None => None,
        })
        .collect();

    let mut out = String::new();
    out.push_str("```ridl\n");
    if inline.is_some() {
        out.push_str(&format!("service {} {{ … }}", service.name));
    } else {
        out.push_str(&format!("service {} : {}", service.name, slots.join(", ")));
    }
    out.push_str("\n```");

    match inline {
        Some(shape) => {
            let count = shape.interactions.len();
            let plural = if count == 1 { "" } else { "s" };
            out.push_str(&format!(
                "\n\n**Shape:** inline — {count} interaction{plural}"
            ));
        }
        None if slots.is_empty() => {}
        None => {
            let plural = if slots.len() == 1 { "" } else { "s" };
            out.push_str(&format!(
                "\n\n**Interface{plural}:** {}",
                slots
                    .iter()
                    .map(|slot| format!("`{slot}`"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
    }

    out.push_str(&format!("\n\n{POSTURE_NOTE}"));
    let parts = DocParts {
        doc: &service.doc,
        links: &service.links,
        since: &service.since,
        labels: &service.labels,
        deprecated: service.deprecated.as_deref(),
    };
    push_doc_sections(&mut out, &parts, &[], resolve);
    out
}

/// The comma-separated parameter list of a command or query.
fn params(params: &[v2::Param]) -> String {
    params
        .iter()
        .map(|param| {
            let type_text = param
                .r#type
                .as_ref()
                .map(field_type_text)
                .unwrap_or_else(|| "?".to_string());
            format!("{}: {type_text}", param.name)
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// The source form of a query's return shape (ridl §7).
fn return_text(return_type: &v2::ReturnType) -> String {
    match &return_type.kind {
        Some(v2::return_type::Kind::Value(value)) => field_type_text(value),
        Some(v2::return_type::Kind::Fallible(fallible)) => {
            format!("{} | {}", fallible.ok, fallible.err)
        }
        None => "?".to_string(),
    }
}

/// The inline fallible arms of a return shape, if it has them.
fn fallible_of(return_type: &v2::ReturnType) -> Option<&v2::FallibleType> {
    match &return_type.kind {
        Some(v2::return_type::Kind::Fallible(fallible)) => Some(fallible),
        _ => None,
    }
}

/// The canonical named reference a field type carries, if it is a plain named
/// type — the only shape whose typl detail a payload line can expand.
fn named_ref(field_type: &v2::FieldType) -> Option<&str> {
    match &field_type.kind {
        Some(v2::field_type::Kind::Named(name)) => Some(name),
        _ => None,
    }
}

/// The source form of a field type (typl §7, §11–§12; ridl §12 for streams).
fn field_type_text(field_type: &v2::FieldType) -> String {
    let optional = if field_type.optional { "?" } else { "" };
    let base = match &field_type.kind {
        Some(v2::field_type::Kind::Named(name)) => name.clone(),
        Some(v2::field_type::Kind::Primitive(primitive)) => primitive_name(*primitive).to_string(),
        Some(v2::field_type::Kind::InlineScalar(scalar)) => format!(
            "{}{}",
            backing(scalar.backing.as_ref()).trim_start_matches(" : "),
            constraint(scalar.constraint.as_ref()),
        ),
        Some(v2::field_type::Kind::Tuple(tuple)) => format!(
            "({})",
            tuple
                .fields
                .iter()
                .map(|field| {
                    let type_text = field
                        .r#type
                        .as_ref()
                        .map(field_type_text)
                        .unwrap_or_else(|| "?".to_string());
                    format!("{}: {type_text}", field.name)
                })
                .collect::<Vec<_>>()
                .join(", "),
        ),
        Some(v2::field_type::Kind::Array(array)) => format!(
            "[{}; {}..{}]",
            array
                .element
                .as_ref()
                .map(|element| field_type_text(element))
                .unwrap_or_else(|| "?".to_string()),
            array.min,
            array.max,
        ),
        Some(v2::field_type::Kind::Map(map)) => format!(
            "{{{}: {}; {}..{}}}",
            map.key
                .as_ref()
                .map(|key| field_type_text(key))
                .unwrap_or_else(|| "?".to_string()),
            map.value
                .as_ref()
                .map(|value| field_type_text(value))
                .unwrap_or_else(|| "?".to_string()),
            map.min,
            map.max,
        ),
        Some(v2::field_type::Kind::Stream(stream)) => match &stream.element {
            Some(v2::stream_type::Element::Named(name)) => format!("<{name}>"),
            Some(v2::stream_type::Element::Primitive(primitive)) => {
                format!("<{}>", primitive_name(*primitive))
            }
            None => "<?>".to_string(),
        },
        None => "?".to_string(),
    };
    format!("{base}{optional}")
}

/// The declaration a canonical type reference names, looked up through the
/// memoized check query of whichever package owns it. A bare reference is
/// same-package by the IR's canonical-form rule.
fn decl_of_ref(
    db: &dyn salsa::Database,
    ws: Workspace,
    std: Package,
    pkg: Package,
    canonical: &str,
) -> Option<v2::Decl> {
    let (package_path, name) = match canonical.rsplit_once('.') {
        Some((path, name)) => (path.to_string(), name.to_string()),
        None => (pkg.name(db).clone(), canonical.to_string()),
    };
    let target = owner_package(db, ws, std, pkg, &package_path)?;
    check_package(db, ws, target, std)
        .ir
        .decls
        .into_iter()
        .find(|decl| decl.name == name)
}

/// The compact typl detail of a declaration used as a payload: the backing and
/// constraint of a scalar type, or the shape word of a composite.
fn type_summary(decl: &v2::Decl) -> Option<String> {
    match &decl.kind {
        Some(v2::decl::Kind::TypeDef(type_def)) => {
            let text = format!(
                "{}{}",
                backing(type_def.backing.as_ref()).trim_start_matches(" : "),
                constraint(type_def.constraint.as_ref()),
            );
            let text = text.trim();
            (!text.is_empty()).then(|| format!("`{text}`"))
        }
        Some(v2::decl::Kind::StructDef(_)) => Some("struct".to_string()),
        Some(v2::decl::Kind::EnumDef(_)) => Some("enum".to_string()),
        Some(v2::decl::Kind::EnumSetDef(_)) => Some("enumset".to_string()),
        Some(v2::decl::Kind::UnionDef(_)) => Some("union".to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn link(text: &str, offset: u32, len: u32, target: &str) -> v2::DocLink {
        v2::DocLink {
            text: text.to_string(),
            offset,
            len,
            target: target.to_string(),
        }
    }

    fn location(line: u32) -> lt::Location {
        lt::Location {
            uri: convert::path_to_uri("/w/lib.typl").expect("an absolute path"),
            range: lt::Range::new(lt::Position::new(line, 5), lt::Position::new(line, 10)),
        }
    }

    #[test]
    fn render_doc_links_a_resolved_target_and_spans_an_unresolved_one() {
        let doc = "See [Speed] and [`Gear`].";
        let links = [
            link("Speed", 4, 7, "veh.Speed"),
            link("`Gear`", 16, 8, "veh.Gear"),
        ];
        let rendered = render_doc(doc, &links, |target| {
            (target == "veh.Speed").then(|| location(2))
        });
        assert_eq!(rendered, "See [Speed](file:///w/lib.typl#L3) and `Gear`.");
    }

    #[test]
    fn render_doc_leaves_a_link_off_a_character_boundary_as_written() {
        // `ö` is two bytes: offset 1 is inside it.
        let doc = "ö [A]";
        let links = [link("A", 1, 3, "veh.A")];
        assert_eq!(render_doc(doc, &links, |_| Some(location(0))), doc);
    }

    fn defaulted_range(min: Option<&str>, max: &str) -> v2::Timing {
        v2::Timing {
            mode: v2::TimingMode::Range as i32,
            min_us: min.map(str::to_string),
            max_us: Some(max.to_string()),
            default_applied: true,
        }
    }

    /// On an RPC with a minimum, `default_applied` means at least the maximum
    /// came from the default, so the line names only the maximum as defaulted
    /// and never calls a written `@[20ms..]` the default.
    #[test]
    fn timing_line_names_only_the_maximum_as_defaulted_on_an_rpc_with_a_minimum() {
        let line = timing_line(
            &defaulted_range(Some("20000"), "1000000"),
            Reading::Acceptance,
        );
        assert!(
            line.contains("`[20ms..1000ms]` (maximum 1s taken from the default)"),
            "{line}"
        );
        assert!(!line.contains("default [20ms"), "{line}");
    }

    /// The same wording holds on a query, whose reading is the reply and not
    /// the acceptance.
    #[test]
    fn timing_line_names_only_the_maximum_as_defaulted_on_a_query_with_a_minimum() {
        let line = timing_line(&defaulted_range(Some("20000"), "3000000"), Reading::Reply);
        assert!(
            line.contains("`[20ms..3000ms]` (maximum 3s taken from the default)"),
            "{line}"
        );
        assert!(!line.contains("default [20ms"), "{line}");
    }

    /// With no minimum the whole range is the default, so the line says the
    /// default was applied, on an RPC and on a signal alike.
    #[test]
    fn timing_line_names_the_whole_default_without_a_minimum() {
        let line = timing_line(&defaulted_range(None, "1000000"), Reading::Acceptance);
        assert!(line.contains("(default [..1s] applied)"), "{line}");
        let line = timing_line(&defaulted_range(Some("100000"), "1000000"), Reading::State);
        assert!(line.contains("(default [100ms..1000ms] applied)"), "{line}");
    }

    #[test]
    fn render_doc_skips_a_see_entry() {
        let links = [link("Speed", 0, 0, "veh.Speed")];
        assert_eq!(render_doc("Text.", &links, |_| Some(location(0))), "Text.");
    }
}
