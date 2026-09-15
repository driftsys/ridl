//! The attribute blocks of the rsdl declarations and lines (rsdl reference §5).
//!
//! An rsdl-owned key is allow-listed per declaration kind and has a consumer;
//! every other key belongs to a backend and is carried uninterpreted. A key no
//! row of the §5 table and no backend namespace defines is FORM-106, an
//! rsdl-owned key where its row does not allow it is FORM-107, and a key twice
//! in one block is FORM-108. The value rules of the rsdl-owned keys are
//! RSDL-305 (`instances`), RSDL-313 (`external`) and RSDL-908 (`tier`).
//!
//! The ridl member check (`check::Checker::check_member_attrs`) raises the same
//! three FORM codes, but it is a method of the package checker, bound to the
//! interaction kinds and to package-relative file ids, so it is not called
//! from here. The messages keep its wording.

use std::collections::HashSet;

use ridl_core::db::InputFile;
use ridl_core::diag::DiagCode;
use ridl_syntax::ast::{self, AstNode};

use super::{
    BackendKey, DeclAttrs, Named, Reporter, Site, Tier, UNIT_INSTANCE, WrittenValue,
    is_lower_camel, is_screaming_snake,
};
use crate::resolve::significant_text;

/// Where an attribute block sits (rsdl §3 table): one of the five declaration
/// kinds, or a line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AttrSite {
    System,
    Component,
    Distribution,
    Deployment,
    Machine,
    Line,
}

impl AttrSite {
    fn noun(self) -> &'static str {
        match self {
            Self::System => "a `system`",
            Self::Component => "a `component`",
            Self::Distribution => "a `distribution`",
            Self::Deployment => "a `deployment`",
            Self::Machine => "a `machine`",
            Self::Line => "a line",
        }
    }

    /// Whether the rsdl-owned `key` is legal here (the rsdl §5 table).
    fn allows(self, key: &str) -> bool {
        match key {
            "instances" => self == Self::Component,
            "external" => matches!(self, Self::Component | Self::Machine),
            "tier" => self == Self::Distribution,
            "labels" | "deprecated" => self != Self::Line,
            _ => false,
        }
    }
}

/// The rsdl-owned keys (rsdl §5 table).
const RSDL_KEYS: [&str; 5] = ["instances", "external", "tier", "labels", "deprecated"];

/// What one attribute block yields. A line uses `attrs.backend_keys` only: its
/// other fields stay empty, because every rsdl-owned key on a line is FORM-107.
#[derive(Debug, Default)]
pub(super) struct ReadAttrs {
    pub(super) attrs: DeclAttrs,
    pub(super) instances: Option<Vec<Named>>,
    pub(super) external: bool,
    pub(super) tier: Option<Tier>,
}

/// Reads `block` at `at`, reporting every key rule of rsdl §5.
pub(super) fn read(
    block: Option<ast::AttrBlock>,
    at: AttrSite,
    file: InputFile,
    reporter: &mut Reporter,
) -> ReadAttrs {
    let mut out = ReadAttrs::default();
    let Some(block) = block else {
        return out;
    };
    let mut seen: HashSet<String> = HashSet::new();
    for attribute in block.attributes() {
        let site = Site {
            file,
            range: attribute.syntax().text_range(),
        };
        let segments: Vec<String> = attribute
            .key_segments()
            .filter_map(|name| Some(name.ident_token()?.text().to_string()))
            .collect();
        // A key the parser could not read — a reserved word, a dangling `.` —
        // already drew its parse error.
        let key = match (segments.as_slice(), attribute.dot_token().is_some()) {
            ([key], false) => key.clone(),
            ([namespace, key], true) => format!("{namespace}.{key}"),
            _ => continue,
        };
        if !seen.insert(key.clone()) {
            reporter.error(
                DiagCode::FORM_108,
                site,
                format!("duplicate attribute key `{key}`"),
            );
            continue;
        }
        if let [namespace, key_name] = segments.as_slice() {
            if is_lower_camel(namespace) && is_lower_camel(key_name) {
                out.attrs.backend_keys.push(BackendKey {
                    namespace: namespace.clone(),
                    key: key_name.clone(),
                    value: attribute.value().map(|value| written(&value)),
                    site,
                });
            } else {
                reporter.error(
                    DiagCode::FORM_106,
                    site,
                    format!(
                        "unknown attribute key `{key}` — a backend key is a camelCase namespace, \
                         a dot and a camelCase key (rsdl reference §5)"
                    ),
                );
            }
            continue;
        }
        if !RSDL_KEYS.contains(&key.as_str()) {
            reporter.error(
                DiagCode::FORM_106,
                site,
                format!("unknown attribute key `{key}`"),
            );
            continue;
        }
        if !at.allows(&key) {
            let message = if at == AttrSite::Line {
                format!(
                    "attribute `{key}` not valid on a line — a line takes backend keys only \
                     (rsdl reference §5)"
                )
            } else {
                format!(
                    "attribute `{key}` not valid on {} (rsdl reference §5)",
                    at.noun()
                )
            };
            reporter.error(DiagCode::FORM_107, site, message);
            continue;
        }
        let value = attribute.value();
        match key.as_str() {
            "instances" => out.instances = Some(instances(value, site, file, reporter)),
            "external" => {
                if value.is_some() {
                    reporter.error(
                        DiagCode::RSDL_313,
                        site,
                        "`external` is a flag and takes no value — write `[ external ]` \
                         (rsdl reference §5)"
                            .to_string(),
                    );
                }
                out.external = true;
            }
            "tier" => out.tier = tier(value, site, reporter),
            "labels" => out.attrs.labels = labels(value, site, reporter),
            "deprecated" => out.attrs.deprecated = deprecated(value, site, reporter),
            _ => {}
        }
    }
    out
}

/// `instances = (a, b, …)` (rsdl §7): the camelCase names, and a written
/// `Unit`, which RSDL-307 reports in the closure check. A value that is not a
/// parenthesised list, an empty list, and an item that is not a camelCase
/// name are RSDL-305.
fn instances(
    value: Option<ast::AttrValue>,
    site: Site,
    file: InputFile,
    reporter: &mut Reporter,
) -> Vec<Named> {
    const RULE: &str = "`instances` takes a parenthesised list of one or more camelCase names, \
                        such as `instances = (primary, backup)` (rsdl reference §7)";
    let Some(list) = value.filter(|value| value.l_paren_token().is_some()) else {
        reporter.error(DiagCode::RSDL_305, site, RULE.to_string());
        return Vec::new();
    };
    let items: Vec<ast::AttrValue> = list.values().collect();
    if items.is_empty() {
        reporter.error(DiagCode::RSDL_305, site, RULE.to_string());
        return Vec::new();
    }
    let mut names = Vec::new();
    for item in items {
        let item_site = Site {
            file,
            range: item.syntax().text_range(),
        };
        match bare_name(&item) {
            Some(name) if is_lower_camel(&name) || name == UNIT_INSTANCE => names.push(Named {
                name,
                site: item_site,
            }),
            _ => reporter.error(
                DiagCode::RSDL_305,
                item_site,
                format!(
                    "`{}` is not a camelCase instance name — {RULE}",
                    significant_text(item.syntax())
                ),
            ),
        }
    }
    names
}

/// `tier = PLATFORM` or `tier = APPLICATION` (rsdl §3.3); any other value,
/// or none, is RSDL-908.
fn tier(value: Option<ast::AttrValue>, site: Site, reporter: &mut Reporter) -> Option<Tier> {
    match value.as_ref().and_then(bare_name).as_deref() {
        Some("PLATFORM") => Some(Tier::Platform),
        Some("APPLICATION") => Some(Tier::Application),
        _ => {
            reporter.error(
                DiagCode::RSDL_908,
                site,
                "`tier` is `PLATFORM` or `APPLICATION` (rsdl reference §3.3)".to_string(),
            );
            None
        }
    }
}

/// `labels = (LABEL, …)` (rsdl §5, general form §4.7). A value of another shape
/// draws FORM-101 — the rsdl §16 table mints no code for it — and is dropped.
fn labels(value: Option<ast::AttrValue>, site: Site, reporter: &mut Reporter) -> Vec<String> {
    let labels: Option<Vec<String>> = value
        .filter(|value| value.l_paren_token().is_some())
        .and_then(|list| {
            list.values()
                .map(|item| bare_name(&item).filter(|name| is_screaming_snake(name)))
                .collect()
        });
    match labels {
        Some(labels) if !labels.is_empty() => labels,
        _ => {
            reporter.error(
                DiagCode::FORM_101,
                site,
                "expected `labels = (LABEL, …)`, a parenthesised list of SCREAMING_SNAKE \
                 labels (rsdl reference §5)"
                    .to_string(),
            );
            Vec::new()
        }
    }
}

/// `deprecated = "reason"` (rsdl §5, general form §4.7): the text between the
/// quotes. A value of another shape draws FORM-101 and is dropped.
fn deprecated(
    value: Option<ast::AttrValue>,
    site: Site,
    reporter: &mut Reporter,
) -> Option<String> {
    let reason = value
        .and_then(|value| value.literal())
        .and_then(|literal| literal.string_lit_token())
        .map(|token| {
            let text = token.text();
            text.strip_prefix('"')
                .and_then(|rest| rest.strip_suffix('"'))
                .unwrap_or(text)
                .to_string()
        });
    if reason.is_none() {
        reporter.error(
            DiagCode::FORM_101,
            site,
            "expected `deprecated = \"reason\"`, a string literal (rsdl reference §5)".to_string(),
        );
    }
    reason
}

/// The name a single-literal value holds — `solo`, `PLATFORM` — or `None` for
/// a list, a number, a string, or a negated name.
fn bare_name(value: &ast::AttrValue) -> Option<String> {
    let literal = value.literal()?;
    if literal.minus_token().is_some() {
        return None;
    }
    Some(literal.ident_token()?.text().to_string())
}

/// A value as written (rsdl §5: carried, never interpreted).
fn written(value: &ast::AttrValue) -> WrittenValue {
    if value.l_paren_token().is_some() {
        WrittenValue::List(value.values().map(|item| written(&item)).collect())
    } else {
        WrittenValue::Scalar(
            value
                .literal()
                .map(|literal| significant_text(literal.syntax()))
                .unwrap_or_default(),
        )
    }
}
