//! The doc fields of the model: the resolved doc links and `see` targets
//! (ADR-0026), and the rules of an item (ADR-0028).
//!
//! The lowering copies `links`, `see` and `since` from the IR item a carrier
//! is lowered from, and fills `rules` from [`crate::rules::rules`], so every
//! backend and plugin reads the documented contract from the model and none
//! re-derives it. Each rule's `text` is [`crate::rules::render`] of that rule,
//! the line the language server's hover shows.

use super::v1;
use crate::rules::{self, Rule};
use crate::v2;

/// The IR's doc links, field by field, in the same order.
pub(crate) fn doc_links(links: &[v2::DocLink]) -> Vec<v1::DocLink> {
    links
        .iter()
        .map(|link| v1::DocLink {
            text: link.text.clone(),
            offset: link.offset,
            len: link.len,
            target: link.target.clone(),
        })
        .collect()
}

/// One rule, with its structured fields and its canonical text line. The
/// timing mode and the contract kind keep their IR values, which the model's
/// `TimingMode` and `ContractKind` restate.
pub(crate) fn model_rule(rule: &Rule) -> v1::Rule {
    use v1::rule::Kind;
    let kind = match rule {
        Rule::Range { min, max } => Kind::Range(v1::rule::Range {
            min: min.clone(),
            max: max.clone(),
        }),
        Rule::Step(step) => Kind::Step(step.clone()),
        Rule::Unit(unit) => Kind::Unit(unit.clone()),
        Rule::Length { min, max } => Kind::Length(v1::rule::Length {
            min: *min,
            max: *max,
        }),
        Rule::Pattern(pattern) => Kind::Pattern(pattern.clone()),
        Rule::CollectionBound { min, max } => Kind::CollectionBound(v1::rule::CollectionBound {
            min: *min,
            max: *max,
        }),
        Rule::Timing {
            mode,
            min_us,
            max_us,
        } => Kind::Timing(v1::rule::Timing {
            mode: *mode as i32,
            min_us: min_us.clone(),
            max_us: max_us.clone(),
        }),
        Rule::ResponseBound { min_us, max_us } => Kind::ResponseBound(v1::rule::ResponseBound {
            min_us: min_us.clone(),
            max_us: max_us.clone(),
        }),
        Rule::Errors(names) => Kind::Errors(v1::rule::Errors {
            names: names.clone(),
        }),
        Rule::Contract { kind, source } => Kind::Contract(v1::rule::Contract {
            kind: *kind as i32,
            source: source.clone(),
        }),
    };
    v1::Rule {
        kind: Some(kind),
        text: rules::render(rule),
    }
}

/// The rules of `item`, in the order [`rules::rules`] returns them. `home` is
/// the package that holds `item`; `deps` is the lowered package followed by
/// the others the lowering was handed.
pub(crate) fn model_rules(
    home: &v2::Package,
    deps: &[&v2::Package],
    item: rules::ItemRef<'_>,
) -> Vec<v1::Rule> {
    rules::rules(home, deps, item)
        .iter()
        .map(model_rule)
        .collect()
}
