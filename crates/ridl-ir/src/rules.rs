//! The rules of a declaration, field or parameter: the facts the IR already
//! holds about the values it admits and the way it is used (ADR-0026).
//!
//! [`rules`] reads one item of IR v2 and lists those facts: the value
//! constraint (range, step, unit, length, pattern), a collection bound, the
//! timing of a `signal` or `event`, the declared bounds of a `command` or
//! `query`, the error arm of a fallible `query` return, and each `require` and
//! `ensure` clause as source text. For a field or parameter whose type is a
//! named declaration, the rules are that declaration's rules. [`render`] gives
//! each rule as one short line of text, for a Markdown list in an editor hover.
//!
//! The function reads the IR and adds nothing to it. The shape of [`Rule`] is
//! provisional: it is an input to spec 2b, which decides whether the IR stores
//! the list or each backend calls this function, and may change the shape when
//! it does. Until then, hover in the language server is its only consumer.

use crate::v2;

/// One IR item that [`rules`] can describe.
#[derive(Debug, Clone, Copy)]
pub enum ItemRef<'a> {
    /// A package-level declaration, or an interaction of an interface.
    Decl(&'a v2::Decl),
    /// A struct field.
    Field(&'a v2::Field),
    /// A command or query parameter.
    Param(&'a v2::Param),
}

/// One fact about an item, read from the IR. Numbers are the IR's canonical
/// decimal text, unchanged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rule {
    /// The inclusive value range (typl §5.5); a side is `None` when unstated.
    Range {
        min: Option<String>,
        max: Option<String>,
    },
    /// The quantization step (typl §4.3).
    Step(String),
    /// The canonical UCUM unit of a unit type (typl §5.1).
    Unit(String),
    /// The length bounds of a string, in characters (typl §5.3), or of bytes,
    /// in bytes (typl §5.4); `Constraint.len_min` and `len_max`.
    Length { min: Option<u64>, max: Option<u64> },
    /// The regex source text of a string's `match` pattern (typl §5.3).
    Pattern(String),
    /// The element bound of an array, or the entry bound of a map
    /// (typl §12.1, §12.2). Both bounds are mandatory in the IR.
    CollectionBound { min: u64, max: u64 },
    /// The resolved timing of a `signal` or `event` (ridl §9): the bounds are
    /// exact decimal microsecond text, a side `None` when unstated.
    Timing {
        mode: v2::TimingMode,
        min_us: Option<String>,
        max_us: Option<String>,
    },
    /// The declared bounds of a `command` or `query` (ridl §9): `min_us` is
    /// the call throttle and `max_us` the response bound, as exact decimal
    /// microsecond text.
    ResponseBound {
        min_us: Option<String>,
        max_us: Option<String>,
    },
    /// The qualified names of the error types of an interaction (ridl §10).
    Errors(Vec<String>),
    /// One `require` or `ensure` clause, as its canonical source text
    /// (ridl §13).
    Contract {
        kind: v2::ContractKind,
        source: String,
    },
}

/// The rules of `item`. `pkg` is the package that holds `item`; `deps` are
/// the packages a named type of `item` may come from. A named type that is
/// found in neither gives no rules.
pub fn rules(pkg: &v2::Package, deps: &[&v2::Package], item: ItemRef<'_>) -> Vec<Rule> {
    let mut out = Vec::new();
    match item {
        ItemRef::Decl(decl) => decl_rules(pkg, decl, &mut out),
        ItemRef::Field(field) => field_type_rules(pkg, deps, field.r#type.as_ref(), &mut out),
        ItemRef::Param(param) => field_type_rules(pkg, deps, param.r#type.as_ref(), &mut out),
    }
    out
}

/// The rules a declaration holds itself. A type definition holds no reference
/// to another type, so nothing here follows a reference.
fn decl_rules(pkg: &v2::Package, decl: &v2::Decl, out: &mut Vec<Rule>) {
    use v2::decl::Kind;
    match &decl.kind {
        Some(Kind::TypeDef(type_def)) => type_def_rules(type_def, out),
        Some(Kind::SignalDef(signal)) => timing_rule(signal.timing.as_ref(), out),
        Some(Kind::EventDef(event)) => timing_rule(event.timing.as_ref(), out),
        Some(Kind::CommandDef(command)) => {
            response_bound_rule(command.timing.as_ref(), out);
            contract_rules(&command.contracts, out);
        }
        Some(Kind::QueryDef(query)) => {
            response_bound_rule(query.timing.as_ref(), out);
            if let Some(v2::ReturnType {
                kind: Some(v2::return_type::Kind::Fallible(fallible)),
            }) = &query.return_type
            {
                out.push(Rule::Errors(vec![qualify(&pkg.name, &fallible.err)]));
            }
            contract_rules(&query.contracts, out);
        }
        _ => {}
    }
}

/// The rules of a field or parameter type. A named type gives the rules of
/// the declaration it names; an array or a map gives its bound.
fn field_type_rules(
    pkg: &v2::Package,
    deps: &[&v2::Package],
    field_type: Option<&v2::FieldType>,
    out: &mut Vec<Rule>,
) {
    use v2::field_type::Kind;
    match field_type.and_then(|t| t.kind.as_ref()) {
        Some(Kind::Named(reference)) => {
            if let Some((owner, decl)) = find_decl(pkg, deps, reference) {
                decl_rules(owner, decl, out);
            }
        }
        Some(Kind::InlineScalar(type_def)) => type_def_rules(type_def, out),
        Some(Kind::Array(array)) => {
            out.push(Rule::CollectionBound {
                min: array.min,
                max: array.max,
            });
        }
        Some(Kind::Map(map)) => out.push(Rule::CollectionBound {
            min: map.min,
            max: map.max,
        }),
        _ => {}
    }
}

/// The declaration a named-type reference names, with the package that holds
/// it. A bare reference is same-package; a dotted one is `package.Name`.
fn find_decl<'a>(
    pkg: &'a v2::Package,
    deps: &[&'a v2::Package],
    reference: &str,
) -> Option<(&'a v2::Package, &'a v2::Decl)> {
    let (owner, name) = match reference.rsplit_once('.') {
        None => (pkg, reference),
        Some((package, name)) if package == pkg.name => (pkg, name),
        Some((package, name)) => (*deps.iter().find(|dep| dep.name == package)?, name),
    };
    owner
        .decls
        .iter()
        .find(|decl| decl.name == name)
        .map(|decl| (owner, decl))
}

/// The IR stores a same-package reference bare; a rule names it qualified.
fn qualify(package: &str, reference: &str) -> String {
    if reference.contains('.') || package.is_empty() {
        reference.to_string()
    } else {
        format!("{package}.{reference}")
    }
}

fn type_def_rules(type_def: &v2::TypeDef, out: &mut Vec<Rule>) {
    if let Some(c) = &type_def.constraint {
        if c.min.is_some() || c.max.is_some() {
            out.push(Rule::Range {
                min: c.min.clone(),
                max: c.max.clone(),
            });
        }
        if let Some(step) = &c.step {
            out.push(Rule::Step(step.clone()));
        }
    }
    if let Some(v2::Backing {
        kind: Some(v2::backing::Kind::Unit(unit)),
    }) = &type_def.backing
    {
        out.push(Rule::Unit(unit.clone()));
    }
    if let Some(c) = &type_def.constraint {
        if c.len_min.is_some() || c.len_max.is_some() {
            out.push(Rule::Length {
                min: c.len_min,
                max: c.len_max,
            });
        }
        if let Some(pattern) = &c.pattern {
            out.push(Rule::Pattern(pattern.clone()));
        }
    }
}

fn timing_rule(timing: Option<&v2::Timing>, out: &mut Vec<Rule>) {
    if let Some(t) = timing {
        out.push(Rule::Timing {
            mode: t.mode(),
            min_us: t.min_us.clone(),
            max_us: t.max_us.clone(),
        });
    }
}

fn response_bound_rule(timing: Option<&v2::Timing>, out: &mut Vec<Rule>) {
    if let Some(t) = timing {
        out.push(Rule::ResponseBound {
            min_us: t.min_us.clone(),
            max_us: t.max_us.clone(),
        });
    }
}

fn contract_rules(contracts: &[v2::Contract], out: &mut Vec<Rule>) {
    out.extend(contracts.iter().map(|c| Rule::Contract {
        kind: c.kind(),
        source: c.source.clone(),
    }));
}

/// One rule as one short line of text, for a Markdown list item. A pattern
/// and a contract clause are code spans; every other number or name is
/// plain text, as the IR spells it.
pub fn render(rule: &Rule) -> String {
    match rule {
        Rule::Range { min, max } => format!("range {}", bounds(min.as_deref(), max.as_deref())),
        Rule::Step(step) => format!("step {step}"),
        Rule::Unit(unit) => format!("unit {unit}"),
        Rule::Length { min, max } => {
            format!("length {}", count_bounds(*min, *max))
        }
        Rule::Pattern(pattern) => format!("pattern {}", code_span(pattern)),
        Rule::CollectionBound { min, max } => {
            format!("count {}", count_bounds(Some(*min), Some(*max)))
        }
        Rule::Timing {
            mode: v2::TimingMode::StrictPeriodic,
            min_us,
            max_us,
        } => {
            // Strict periodic stores the period in both bounds (ridl §9).
            let period = min_us.as_deref().or(max_us.as_deref()).unwrap_or("");
            format!("period {period} us")
        }
        Rule::Timing { min_us, max_us, .. } => {
            format!(
                "interval {} us",
                bounds(min_us.as_deref(), max_us.as_deref())
            )
        }
        Rule::ResponseBound { min_us, max_us } => {
            let throttle = min_us
                .as_ref()
                .map(|min| format!("calls at least {min} us apart"));
            let response = max_us
                .as_ref()
                .map(|max| format!("response within {max} us"));
            throttle
                .into_iter()
                .chain(response)
                .collect::<Vec<_>>()
                .join(", ")
        }
        Rule::Errors(names) => format!("errors {}", names.join(", ")),
        Rule::Contract { kind, source } => {
            let keyword = match kind {
                v2::ContractKind::Ensure => "ensure",
                _ => "require",
            };
            format!("{keyword} {}", code_span(source))
        }
    }
}

/// `min ..= max`, `min ..` or `..= max`.
fn bounds(min: Option<&str>, max: Option<&str>) -> String {
    match (min, max) {
        (Some(min), Some(max)) => format!("{min} ..= {max}"),
        (Some(min), None) => format!("{min} .."),
        (None, Some(max)) => format!("..= {max}"),
        (None, None) => "..".to_string(),
    }
}

/// [`bounds`] for whole counts, or the one count when both bounds are equal.
fn count_bounds(min: Option<u64>, max: Option<u64>) -> String {
    match (min, max) {
        (Some(min), Some(max)) if min == max => min.to_string(),
        _ => bounds(
            min.map(|n| n.to_string()).as_deref(),
            max.map(|n| n.to_string()).as_deref(),
        ),
    }
}

/// A Markdown code span around `text`, fenced with one more backtick than the
/// longest run of backticks inside it (CommonMark §6.1).
fn code_span(text: &str) -> String {
    let longest = text.split(|c| c != '`').map(str::len).max().unwrap_or(0);
    let fence = "`".repeat(longest + 1);
    let pad = if text.starts_with('`') || text.ends_with('`') {
        " "
    } else {
        ""
    };
    format!("{fence}{pad}{text}{pad}{fence}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use v2::decl::Kind;

    fn decl(name: &str, kind: Kind) -> v2::Decl {
        v2::Decl {
            name: name.to_string(),
            kind: Some(kind),
            ..Default::default()
        }
    }

    fn package(name: &str, decls: Vec<v2::Decl>) -> v2::Package {
        v2::Package {
            name: name.to_string(),
            decls,
            ..Default::default()
        }
    }

    fn named(reference: &str) -> v2::FieldType {
        v2::FieldType {
            kind: Some(v2::field_type::Kind::Named(reference.to_string())),
            ..Default::default()
        }
    }

    fn field(name: &str, field_type: v2::FieldType) -> v2::Field {
        v2::Field {
            name: name.to_string(),
            r#type: Some(field_type),
            ..Default::default()
        }
    }

    fn some(text: &str) -> Option<String> {
        Some(text.to_string())
    }

    /// `type S : m [0..250 step 0.5]`.
    fn unit_type(name: &str) -> v2::Decl {
        decl(
            name,
            Kind::TypeDef(v2::TypeDef {
                backing: Some(v2::Backing {
                    kind: Some(v2::backing::Kind::Unit("m".into())),
                }),
                constraint: Some(v2::Constraint {
                    min: some("0"),
                    max: some("250"),
                    step: some("0.5"),
                    ..Default::default()
                }),
                ..Default::default()
            }),
        )
    }

    fn unit_type_rules() -> Vec<Rule> {
        vec![
            Rule::Range {
                min: some("0"),
                max: some("250"),
            },
            Rule::Step("0.5".into()),
            Rule::Unit("m".into()),
        ]
    }

    fn timing(mode: v2::TimingMode, min: Option<&str>, max: Option<&str>) -> v2::Timing {
        v2::Timing {
            mode: mode as i32,
            min_us: min.map(str::to_string),
            max_us: max.map(str::to_string),
            default_applied: false,
        }
    }

    #[test]
    fn unit_type_gives_range_step_and_unit() {
        let s = unit_type("S");
        let pkg = package("app", vec![s.clone()]);
        assert_eq!(rules(&pkg, &[], ItemRef::Decl(&s)), unit_type_rules());
    }

    #[test]
    fn bounded_string_type_gives_length_and_pattern() {
        let vin = decl(
            "Vin",
            Kind::TypeDef(v2::TypeDef {
                backing: Some(v2::Backing {
                    kind: Some(v2::backing::Kind::Primitive(
                        v2::PrimitiveType::String as i32,
                    )),
                }),
                constraint: Some(v2::Constraint {
                    len_min: Some(1),
                    len_max: Some(17),
                    pattern: some("^[A-Z0-9]+$"),
                    pattern_const: some("VIN_PATTERN"),
                    ..Default::default()
                }),
                ..Default::default()
            }),
        );
        let pkg = package("app", vec![vin.clone()]);
        assert_eq!(
            rules(&pkg, &[], ItemRef::Decl(&vin)),
            vec![
                Rule::Length {
                    min: Some(1),
                    max: Some(17)
                },
                Rule::Pattern("^[A-Z0-9]+$".into()),
            ]
        );
    }

    #[test]
    fn array_and_map_fields_give_a_collection_bound() {
        let pkg = package("app", vec![]);
        let array = field(
            "points",
            v2::FieldType {
                kind: Some(v2::field_type::Kind::Array(Box::new(v2::ArrayType {
                    element: Some(Box::new(named("S"))),
                    min: 1,
                    max: 16,
                }))),
                ..Default::default()
            },
        );
        assert_eq!(
            rules(&pkg, &[], ItemRef::Field(&array)),
            vec![Rule::CollectionBound { min: 1, max: 16 }]
        );
        let map = field(
            "table",
            v2::FieldType {
                kind: Some(v2::field_type::Kind::Map(Box::new(v2::MapType {
                    key: Some(Box::new(named("K"))),
                    value: Some(Box::new(named("V"))),
                    min: 0,
                    max: 8,
                }))),
                ..Default::default()
            },
        );
        assert_eq!(
            rules(&pkg, &[], ItemRef::Field(&map)),
            vec![Rule::CollectionBound { min: 0, max: 8 }]
        );
    }

    #[test]
    fn signal_gives_timing() {
        let signal = decl(
            "speed",
            Kind::SignalDef(v2::SignalDef {
                payload: "S".into(),
                timing: Some(timing(v2::TimingMode::Range, Some("1000"), Some("50000"))),
                ..Default::default()
            }),
        );
        let pkg = package("app", vec![]);
        assert_eq!(
            rules(&pkg, &[], ItemRef::Decl(&signal)),
            vec![Rule::Timing {
                mode: v2::TimingMode::Range,
                min_us: some("1000"),
                max_us: some("50000"),
            }]
        );
    }

    #[test]
    fn event_gives_timing() {
        let event = decl(
            "tick",
            Kind::EventDef(v2::EventDef {
                payload: "S".into(),
                timing: Some(timing(
                    v2::TimingMode::StrictPeriodic,
                    Some("10000"),
                    Some("10000"),
                )),
            }),
        );
        let pkg = package("app", vec![]);
        assert_eq!(
            rules(&pkg, &[], ItemRef::Decl(&event)),
            vec![Rule::Timing {
                mode: v2::TimingMode::StrictPeriodic,
                min_us: some("10000"),
                max_us: some("10000"),
            }]
        );
    }

    #[test]
    fn command_with_a_declared_bound_gives_response_bound_and_contract() {
        let command = decl(
            "setSpeed",
            Kind::CommandDef(v2::CommandDef {
                timing: Some(timing(v2::TimingMode::Range, None, Some("20000"))),
                contracts: vec![v2::Contract {
                    kind: v2::ContractKind::Require as i32,
                    source: "target >= 0".into(),
                    ..Default::default()
                }],
                ..Default::default()
            }),
        );
        let pkg = package("app", vec![]);
        assert_eq!(
            rules(&pkg, &[], ItemRef::Decl(&command)),
            vec![
                Rule::ResponseBound {
                    min_us: None,
                    max_us: some("20000")
                },
                Rule::Contract {
                    kind: v2::ContractKind::Require,
                    source: "target >= 0".into()
                },
            ]
        );
    }

    #[test]
    fn command_without_a_declared_bound_gives_no_response_bound() {
        let command = decl("reset", Kind::CommandDef(v2::CommandDef::default()));
        let pkg = package("app", vec![]);
        assert_eq!(rules(&pkg, &[], ItemRef::Decl(&command)), vec![]);
    }

    #[test]
    fn fallible_query_gives_qualified_errors_and_contracts() {
        let query = decl(
            "read",
            Kind::QueryDef(v2::QueryDef {
                return_type: Some(v2::ReturnType {
                    kind: Some(v2::return_type::Kind::Fallible(v2::FallibleType {
                        ok: "T".into(),
                        err: "E".into(),
                    })),
                }),
                contracts: vec![v2::Contract {
                    kind: v2::ContractKind::Ensure as i32,
                    source: "result > 0".into(),
                    ..Default::default()
                }],
                timing: Some(timing(v2::TimingMode::Range, Some("500"), Some("2000"))),
                ..Default::default()
            }),
        );
        let pkg = package("app", vec![]);
        assert_eq!(
            rules(&pkg, &[], ItemRef::Decl(&query)),
            vec![
                Rule::ResponseBound {
                    min_us: some("500"),
                    max_us: some("2000")
                },
                Rule::Errors(vec!["app.E".into()]),
                Rule::Contract {
                    kind: v2::ContractKind::Ensure,
                    source: "result > 0".into()
                },
            ]
        );
    }

    #[test]
    fn query_error_in_another_package_keeps_its_qualified_name() {
        let query = decl(
            "read",
            Kind::QueryDef(v2::QueryDef {
                return_type: Some(v2::ReturnType {
                    kind: Some(v2::return_type::Kind::Fallible(v2::FallibleType {
                        ok: "T".into(),
                        err: "lib.err.Fault".into(),
                    })),
                }),
                ..Default::default()
            }),
        );
        let pkg = package("app", vec![]);
        assert_eq!(
            rules(&pkg, &[], ItemRef::Decl(&query)),
            vec![Rule::Errors(vec!["lib.err.Fault".into()])]
        );
    }

    #[test]
    fn field_follows_named_type() {
        let pkg = package("app", vec![unit_type("S")]);
        let f = field("distance", named("S"));
        assert_eq!(rules(&pkg, &[], ItemRef::Field(&f)), unit_type_rules());
    }

    #[test]
    fn param_follows_named_type_into_a_dependency() {
        let dep = package("lib.units", vec![unit_type("Metres")]);
        let pkg = package("app", vec![]);
        let p = v2::Param {
            name: "distance".into(),
            r#type: Some(named("lib.units.Metres")),
            ..Default::default()
        };
        assert_eq!(rules(&pkg, &[&dep], ItemRef::Param(&p)), unit_type_rules());
    }

    #[test]
    fn named_type_that_cannot_be_found_gives_no_rules() {
        let pkg = package("app", vec![]);
        let dep = package("lib.units", vec![]);
        for reference in ["Missing", "lib.units.Missing", "other.Missing"] {
            let f = field("x", named(reference));
            assert_eq!(
                rules(&pkg, &[&dep], ItemRef::Field(&f)),
                vec![],
                "{reference}"
            );
        }
    }

    #[test]
    fn inline_scalar_field_gives_its_constraint() {
        let pkg = package("app", vec![]);
        let f = field(
            "gear",
            v2::FieldType {
                kind: Some(v2::field_type::Kind::InlineScalar(Box::new(v2::TypeDef {
                    backing: Some(v2::Backing {
                        kind: Some(v2::backing::Kind::Primitive(
                            v2::PrimitiveType::Integer as i32,
                        )),
                    }),
                    constraint: Some(v2::Constraint {
                        max: some("6"),
                        ..Default::default()
                    }),
                    ..Default::default()
                }))),
                ..Default::default()
            },
        );
        assert_eq!(
            rules(&pkg, &[], ItemRef::Field(&f)),
            vec![Rule::Range {
                min: None,
                max: some("6")
            }]
        );
    }

    #[test]
    fn render_gives_a_fixed_string_for_each_variant() {
        let cases = [
            (
                Rule::Range {
                    min: some("0"),
                    max: some("250"),
                },
                "range 0 ..= 250",
            ),
            (
                Rule::Range {
                    min: some("-40"),
                    max: None,
                },
                "range -40 ..",
            ),
            (
                Rule::Range {
                    min: None,
                    max: some("6"),
                },
                "range ..= 6",
            ),
            (Rule::Step("0.5".into()), "step 0.5"),
            (Rule::Unit("m".into()), "unit m"),
            (
                Rule::Length {
                    min: Some(1),
                    max: Some(17),
                },
                "length 1 ..= 17",
            ),
            (
                Rule::Length {
                    min: Some(8),
                    max: Some(8),
                },
                "length 8",
            ),
            (
                Rule::Length {
                    min: None,
                    max: Some(64),
                },
                "length ..= 64",
            ),
            (Rule::Pattern("^[A-Z]+$".into()), "pattern `^[A-Z]+$`"),
            (Rule::Pattern("a`b".into()), "pattern ``a`b``"),
            (Rule::CollectionBound { min: 1, max: 16 }, "count 1 ..= 16"),
            (Rule::CollectionBound { min: 4, max: 4 }, "count 4"),
            (
                Rule::Timing {
                    mode: v2::TimingMode::StrictPeriodic,
                    min_us: some("10000"),
                    max_us: some("10000"),
                },
                "period 10000 us",
            ),
            (
                Rule::Timing {
                    mode: v2::TimingMode::Range,
                    min_us: some("1000"),
                    max_us: some("50000"),
                },
                "interval 1000 ..= 50000 us",
            ),
            (
                Rule::Timing {
                    mode: v2::TimingMode::Range,
                    min_us: None,
                    max_us: some("50000"),
                },
                "interval ..= 50000 us",
            ),
            (
                Rule::ResponseBound {
                    min_us: some("500"),
                    max_us: some("2000"),
                },
                "calls at least 500 us apart, response within 2000 us",
            ),
            (
                Rule::ResponseBound {
                    min_us: None,
                    max_us: some("2000"),
                },
                "response within 2000 us",
            ),
            (
                Rule::ResponseBound {
                    min_us: some("500"),
                    max_us: None,
                },
                "calls at least 500 us apart",
            ),
            (Rule::Errors(vec!["app.E".into()]), "errors app.E"),
            (
                Rule::Errors(vec!["app.E".into(), "lib.F".into()]),
                "errors app.E, lib.F",
            ),
            (
                Rule::Contract {
                    kind: v2::ContractKind::Require,
                    source: "x > 0".into(),
                },
                "require `x > 0`",
            ),
            (
                Rule::Contract {
                    kind: v2::ContractKind::Ensure,
                    source: "result > 0".into(),
                },
                "ensure `result > 0`",
            ),
        ];
        for (rule, expected) in cases {
            assert_eq!(render(&rule), expected, "{rule:?}");
        }
    }
}
