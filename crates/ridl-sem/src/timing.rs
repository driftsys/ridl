//! Timing resolution: parse and validate `@` annotations, apply the configured
//! default to untimed signals and events, and produce the resolved bounds the
//! IR carries (ridl language reference §9, general form §6.2, ADR-0008
//! decision 12; E2 task 9).
//!
//! Durations convert to exact microseconds — the five duration suffixes ridl
//! §2.1 tabulates: `us`/`ms`/`s` scale by 1 / 1_000 / 1_000_000, and `min`/`h`
//! by 60_000_000 / 3_600_000_000.
//!
//! The lexer does **not** guarantee a whole-number duration: it merges an
//! `IntNumber` **or** a `FloatNumber` followed by a time atom into one
//! `Duration` token, so `1.5ms` and `0.0ms` reach this module. ridl §2.1 admits
//! only a positive integer followed by a time-unit suffix, so a fractional
//! literal is rejected here with a pointed FORM-102 — and its exact value is
//! still carried into the IR, because a bound that was *written* must never be
//! silently dropped (a dropped bound is an invisible contract change for the
//! `ridl diff` classifier). A bound is `None` only when it is genuinely absent,
//! as in the half-open `@[20ms..]`.
//!
//! Bound meaning is one generic rule (gf §6.2, ADR-0008 decision 1): `min` is
//! the rate floor, `max` the staleness bound. A strict period stores the same
//! value in both bounds; an explicit half-open range leaves the absent side
//! unset; an untimed signal or event resolves the configured default (RIDL-100)
//! so that "untimed" never survives past the parser (ridl §9.1) — the IR always
//! carries concrete bounds and a `default_applied` flag (ADR-0008 decision 12).

use num_bigint::BigInt;
use num_rational::BigRational;
use ridl_core::diag::{DiagCode, Diagnostic, FileId, Severity, Span};
use ridl_syntax::ast::{self, AstNode};
use rowan::TextRange;

use crate::scalar::ExactValue;

/// Whether a resolved timing is a strict period (`@Xms`) or a range (`@[..]`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimingMode {
    StrictPeriodic,
    Range,
}

/// A resolved timing specification in exact microseconds (ridl §9, gf §6.2).
///
/// `min_us` is the rate floor and `max_us` the staleness bound. A strict period
/// stores the same value in both; an explicit half-open range leaves the absent
/// side `None`. `default_applied` marks the configured default that an untimed
/// signal or event resolves to (ADR-0008 decision 12).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimingSpec {
    pub mode: TimingMode,
    pub min_us: Option<ExactValue>,
    pub max_us: Option<ExactValue>,
    pub default_applied: bool,
}

/// The five ridl interaction kinds (ridl §4–§8). Only [`InteractionKind::Signal`]
/// and [`InteractionKind::Event`] carry timing; the other three never do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InteractionKind {
    Signal,
    Event,
    Command,
    Query,
    Final,
}

/// The built-in default timing `[100ms..1000ms]` (ridl §9.1) — the fallback
/// applied to untimed signals and events when no `[defaults].timing` is
/// configured.
pub fn builtin_default_timing() -> TimingSpec {
    parse_default_timing("[100ms..1000ms]")
        .expect("the built-in default `[100ms..1000ms]` is a valid range")
}

/// Parses a `[defaults].timing` string such as `"[100ms..1000ms]"` into a
/// resolved range default (ridl §9.1). The default is always an explicit range
/// with both bounds set, so the applied default always carries concrete,
/// ordered, non-zero bounds — a half-open, zero, or reversed default is
/// rejected. On any malformed input the returned string is the reason the
/// checker renders under MANI-009.
pub fn parse_default_timing(text: &str) -> Result<TimingSpec, String> {
    let trimmed = text.trim();
    let inner = trimmed
        .strip_prefix('[')
        .and_then(|rest| rest.strip_suffix(']'))
        .ok_or_else(|| {
            format!("expected a bracketed range like `[100ms..1000ms]`, found `{text}`")
        })?;
    let (min_text, max_text) = inner
        .split_once("..")
        .ok_or_else(|| format!("expected `min..max`, found `{text}`"))?;
    let min_text = min_text.trim();
    let max_text = max_text.trim();
    if min_text.is_empty() || max_text.is_empty() {
        return Err(format!(
            "the default timing must set both bounds, e.g. `[100ms..1000ms]`, found `{text}`"
        ));
    }
    // A configured default must be whole-number durations too (ridl §2.1); a
    // fractional bound is rejected here rather than carried, because a manifest
    // value has no source span to report a per-bound FORM-102 against.
    let min = whole_bound(min_text)?;
    let max = whole_bound(max_text)?;
    if is_zero(&min) || is_zero(&max) {
        return Err(format!(
            "a timing bound must be greater than zero, found `{text}`"
        ));
    }
    if min > max {
        return Err(format!(
            "the lower bound exceeds the upper bound in `{text}`"
        ));
    }
    Ok(TimingSpec {
        mode: TimingMode::Range,
        min_us: Some(min),
        max_us: Some(max),
        default_applied: false,
    })
}

/// Resolves one interaction's timing to concrete bounds (ridl §9, ADR-0008
/// decision 12).
///
/// For a `signal` or `event` the result is always `Some`: an explicit `@`
/// annotation is parsed and validated, and an untimed one resolves to `default`
/// (RIDL-100). For `command`/`query`/`final` the result is always `None` and no
/// diagnostic is produced — those kinds carry no timing, and the structural
/// checker already reports any annotation written on them (FORM-102 / RIDL-106).
///
/// Validity diagnostics carry `file` as their span source: RIDL-100 (default
/// applied, warning, anchored on `anchor` — the interaction that received the
/// default), RIDL-101 (lower bound exceeds upper), RIDL-102 (zero or negative
/// duration), RIDL-103 (strict `@Xms` on an event), RIDL-108 (`@[X..X]` equal
/// bounds, warning), and FORM-102 for a duration literal that is not a whole
/// number of time units (ridl §2.1). An erroneous annotation still lowers its
/// written bounds, so the IR reflects the source honestly (the E1 discipline)
/// and no written bound is ever silently unset.
pub fn resolve_timing(
    annot: Option<&ast::Timing>,
    kind: InteractionKind,
    default: &TimingSpec,
    file: FileId,
    anchor: TextRange,
) -> (Option<TimingSpec>, Vec<Diagnostic>) {
    if !matches!(kind, InteractionKind::Signal | InteractionKind::Event) {
        return (None, Vec::new());
    }

    let Some(annot) = annot else {
        // Untimed: the configured default is applied (ridl §9.1); RIDL-100
        // warns and names the applied bounds, anchored on the interaction that
        // received the default so a package with many untimed interactions
        // yields one navigable warning each.
        let spec = applied_default(default);
        let diag = diagnostic(
            DiagCode::RIDL_100,
            Severity::Warning,
            file,
            anchor,
            format!(
                "{} without a timing annotation — the default `@{}` is applied: it publishes no \
                 slower than the rate floor and a subscriber may treat a value older than the \
                 staleness bound as stale. Write the bounds this {} really promises (ridl §9.1)",
                kind_noun(kind),
                render_bounds(&spec),
                kind_noun(kind),
            ),
        );
        return (Some(spec), vec![diag]);
    };

    let mut diags = Vec::new();
    if let Some(range) = annot.range() {
        // An explicit range `@[min..max]`, `@[min..]`, or `@[..max]`. A written
        // bound always resolves through `bound_us`, so an illegal duration is
        // reported rather than silently unset; `None` means genuinely absent.
        let min_token = range.min();
        let max_token = range.max();
        let min = bound_us(min_token.as_ref(), file, &mut diags);
        let max = bound_us(max_token.as_ref(), file, &mut diags);
        let node = range.syntax().text_range();
        // The written text of each bound, so every message below quotes the
        // annotation the author typed rather than the microseconds the IR
        // carries.
        let min_text = written(min_token.as_ref());
        let max_text = written(max_token.as_ref());
        let zero = [
            (&min, &min_text, "rate floor"),
            (&max, &max_text, "staleness bound"),
        ]
        .into_iter()
        .find(|(value, _, _)| value.as_ref().is_some_and(is_zero));
        if let Some((_, text, role)) = zero {
            diags.push(error(
                DiagCode::RIDL_102,
                file,
                node,
                format!(
                    "the {role} is `{text}` — a timing bound is a duration greater than zero, \
                     because a bound of zero promises a publication with no delay at all \
                     (ridl §9.2)"
                ),
            ));
        }
        if let (Some(lo), Some(hi)) = (&min, &max) {
            if lo > hi {
                diags.push(error(
                    DiagCode::RIDL_101,
                    file,
                    node,
                    format!(
                        "the rate floor `{min_text}` is longer than the staleness bound \
                         `{max_text}` — in `@[min..max]` the first bound is how often the value \
                         is published and the second how old it may get, so the first must not \
                         exceed the second (ridl §9.2)"
                    ),
                ));
            } else if lo == hi {
                let alternative = match kind {
                    InteractionKind::Signal => format!(
                        ", or write the strict period `@{min_text}` if the rate really is \
                         isochronous — that is a separate mode, not a spelling of this range"
                    ),
                    _ => String::new(),
                };
                diags.push(diagnostic(
                    DiagCode::RIDL_108,
                    Severity::Warning,
                    file,
                    node,
                    format!(
                        "the rate floor and the staleness bound are both `{min_text}` — a \
                         subscriber may then call the value stale the moment the next \
                         publication is due, leaving no margin for jitter. Widen the staleness \
                         bound{alternative} (ridl §9.2)"
                    ),
                ));
            }
        }
        let spec = TimingSpec {
            mode: TimingMode::Range,
            min_us: min,
            max_us: max,
            default_applied: false,
        };
        (Some(spec), diags)
    } else if let Some(token) = annot.duration() {
        // Strict periodic `@Xms` — signal only (ridl §9). The period resolves
        // through `bound_us` so a fractional or unreadable literal is reported,
        // never dropped into an unbounded strict period.
        let value = bound_us(Some(&token), file, &mut diags);
        let node = annot.syntax().text_range();
        if value.as_ref().is_some_and(is_zero) {
            diags.push(error(
                DiagCode::RIDL_102,
                file,
                node,
                format!(
                    "the strict period is `{}` — a period is a duration greater than zero, \
                     because a period of zero promises a publication with no delay at all \
                     (ridl §9.2)",
                    token.text(),
                ),
            ));
        }
        if matches!(kind, InteractionKind::Event) {
            diags.push(error(
                DiagCode::RIDL_103,
                file,
                node,
                format!(
                    "the strict period `@{}` is not valid on an event — a strict period is an \
                     isochronous publication rate, and an event reports occurrences that arrive \
                     when they arrive. Write a range `@[min..max]` instead (ridl §9.2)",
                    token.text(),
                ),
            ));
        }
        let spec = TimingSpec {
            mode: TimingMode::StrictPeriodic,
            min_us: value.clone(),
            max_us: value,
            default_applied: false,
        };
        (Some(spec), diags)
    } else {
        // A degenerate `Timing` node with neither a duration nor a range (a
        // parse error, already reported). Apply the default so the IR still
        // carries concrete bounds.
        (Some(applied_default(default)), diags)
    }
}

/// One bound of a configured `[defaults].timing`, required to be a whole
/// number of time units (ridl §2.1). The error string becomes the MANI-009
/// reason.
fn whole_bound(text: &str) -> Result<ExactValue, String> {
    let duration = duration_us(text).ok_or_else(|| format!("invalid duration `{text}`"))?;
    if !duration.whole {
        return Err(format!(
            "duration `{text}` must be a whole number of us/ms/s/min/h"
        ));
    }
    Ok(duration.us)
}

/// The configured default, cloned and marked as applied (ridl §9.1).
fn applied_default(default: &TimingSpec) -> TimingSpec {
    TimingSpec {
        mode: TimingMode::Range,
        min_us: default.min_us.clone(),
        max_us: default.max_us.clone(),
        default_applied: true,
    }
}

/// One duration literal read from source: its exact microsecond value and
/// whether the written numeric part was a whole number.
///
/// The two are kept apart on purpose. ridl §2.1 admits only a positive integer
/// followed by a time-unit suffix, but the lexer merges a `FloatNumber` with a
/// time atom just as readily as an `IntNumber`, so `1.5ms` arrives here. Its
/// value is still exact (1.5ms is exactly 1500us), so the bound is carried and
/// the caller reports the illegal form — never a silent drop.
struct Duration {
    us: ExactValue,
    whole: bool,
}

/// Converts a duration literal (`"10ms"`, `"2s"`, `"100us"`, `"5min"`, `"1h"`)
/// to exact microseconds.
///
/// Returns `None` only for text that is not a decimal number followed by a
/// known duration suffix (ridl §2.1) — a genuinely unreadable token, which the
/// caller reports rather than dropping. A fractional literal parses and comes
/// back with `whole: false`; scaling is exact rational arithmetic, so no value
/// is rounded on the way in.
fn duration_us(text: &str) -> Option<Duration> {
    let text = text.trim();
    // `us` and `ms` end in `s`, so the two-letter atoms are matched before the
    // bare `s`; `min` before `s` for the same reason of longest-match first.
    let (digits, factor): (&str, u64) = if let Some(digits) = text.strip_suffix("us") {
        (digits, 1)
    } else if let Some(digits) = text.strip_suffix("ms") {
        (digits, 1_000)
    } else if let Some(digits) = text.strip_suffix("min") {
        (digits, 60_000_000)
    } else if let Some(digits) = text.strip_suffix('s') {
        (digits, 1_000_000)
    } else if let Some(digits) = text.strip_suffix('h') {
        (digits, 3_600_000_000)
    } else {
        return None;
    };
    // `ExactValue::parse` accepts an integer or a decimal literal and rejects
    // anything else (a sign, scientific notation, a bare `.`).
    let value = ExactValue::parse(digits)?;
    let whole = !digits.contains('.');
    let micros = value.0 * BigRational::from_integer(BigInt::from(factor));
    Some(Duration {
        us: ExactValue(micros),
        whole,
    })
}

/// The exact microsecond value of a duration literal, for a caller that needs
/// the number and not the timing-annotation rules around it — the contract
/// evaluator (expr-core §7: durations are exact microsecond counts).
///
/// The unit table stays in one place rather than being duplicated: `1s`,
/// `1000ms`, and `1000000us` must be one value in the evaluator exactly as they
/// are in a timing annotation.
pub(crate) fn duration_literal_us(text: &str) -> Option<ExactValue> {
    duration_us(text).map(|duration| duration.us)
}

/// Resolves one written bound token to its exact microsecond value, reporting
/// an illegal duration against the token's own span.
///
/// A duration that is present but not a whole number of time units is FORM-102
/// (ridl §2.1 admits only a positive integer plus a suffix — the T5 precedent
/// for a construct the reference grammar does not admit, so no new RIDL code is
/// burned); a token that cannot be read at all is FORM-102 too. Either way the
/// value is carried when it can be computed, so a bound that was written is
/// never silently unset — `None` here means the bound is genuinely absent.
fn bound_us(
    token: Option<&ridl_syntax::SyntaxToken>,
    file: FileId,
    diags: &mut Vec<Diagnostic>,
) -> Option<ExactValue> {
    let token = token?;
    let span = token.text_range();
    match duration_us(token.text()) {
        Some(duration) => {
            if !duration.whole {
                diags.push(error(
                    DiagCode::FORM_102,
                    file,
                    span,
                    format!(
                        "duration `{}` must be a whole number of us/ms/s/min/h (ridl §2.1)",
                        token.text(),
                    ),
                ));
            }
            Some(duration.us)
        }
        None => {
            diags.push(error(
                DiagCode::FORM_102,
                file,
                span,
                format!("`{}` is not a valid duration (ridl §2.1)", token.text()),
            ));
            None
        }
    }
}

/// Whether an exact microsecond value is zero (RIDL-102).
fn is_zero(value: &ExactValue) -> bool {
    *value.0.numer() == BigInt::from(0)
}

/// The declaring keyword's noun for a diagnostic message.
fn kind_noun(kind: InteractionKind) -> &'static str {
    match kind {
        InteractionKind::Signal => "signal",
        InteractionKind::Event => "event",
        InteractionKind::Command => "command",
        InteractionKind::Query => "query",
        InteractionKind::Final => "final",
    }
}

/// The source text of a written bound token, or `…` when the bound is absent
/// (the half-open forms `@[20ms..]` and `@[..1s]`). Every timing message quotes
/// this rather than a microsecond count: microseconds are the IR's canonical
/// unit and appear in no `.ridl` file.
fn written(token: Option<&ridl_syntax::SyntaxToken>) -> String {
    token.map_or_else(|| "…".to_string(), |token| token.text().to_string())
}

/// Renders a resolved timing as `[<min>..<max>]` for a diagnostic message —
/// each bound as a duration literal, an unset bound rendered as an empty side.
fn render_bounds(spec: &TimingSpec) -> String {
    let render = |bound: &Option<ExactValue>| match bound {
        Some(value) => render_duration(value),
        None => String::new(),
    };
    format!("[{}..{}]", render(&spec.min_us), render(&spec.max_us))
}

/// Renders an exact microsecond count as a duration literal in the largest of
/// the five ridl §2.1 time units that divides it exactly: `100000` renders
/// `100ms` and `1000000` renders `1s`.
///
/// This is used only where nothing was written to echo — the default applied to
/// an untimed signal or event (RIDL-100), which comes from `[defaults].timing`
/// or from the built-in fallback. The units are the ones a `.ridl` file and a
/// manifest are written in, so the message never answers in the canonical
/// microseconds only the IR carries. A value no larger unit divides exactly
/// renders in `us`: a whole count such as `1500` (`1500us`, since `1.5ms` is
/// not a legal duration under ridl §2.1) and a fractional microsecond alike.
fn render_duration(value: &ExactValue) -> String {
    for (suffix, factor) in [
        ("h", 3_600_000_000u64),
        ("min", 60_000_000),
        ("s", 1_000_000),
        ("ms", 1_000),
    ] {
        let scaled = &value.0 / BigRational::from_integer(BigInt::from(factor));
        if scaled.is_integer() {
            return format!("{}{suffix}", scaled.to_integer());
        }
    }
    format!("{}us", value.to_decimal_string())
}

/// Builds a timing [`Diagnostic`]; timing diagnostics carry no secondary labels
/// or fix-its.
fn diagnostic(
    code: DiagCode,
    severity: Severity,
    file: FileId,
    range: TextRange,
    message: String,
) -> Diagnostic {
    Diagnostic {
        code,
        severity,
        message,
        primary: Span { file, range },
        labels: Vec::new(),
        fixits: Vec::new(),
    }
}

/// Builds an error timing [`Diagnostic`].
fn error(code: DiagCode, file: FileId, range: TextRange, message: String) -> Diagnostic {
    diagnostic(code, Severity::Error, file, range, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ridl_core::diag::SourceMap;
    use ridl_syntax::Profile;
    use ridl_syntax::ast::SourceFile;
    use rowan::TextSize;

    /// A throwaway [`FileId`] for the diagnostics under test.
    fn file_id() -> FileId {
        let mut map = SourceMap::new();
        map.file_id("t.ridl", "")
    }

    /// Parses one interaction declaration inside an interface and returns its
    /// timing annotation (the `signal`/`event` timing accessor).
    fn annot(decl: &str) -> ast::Timing {
        let src = format!("package p\ninterface I {{\n  {decl}\n}}\n");
        let parse = ridl_syntax::parse(&src, Profile::Ridl);
        let file = SourceFile::cast(parse.syntax()).expect("root is a SourceFile");
        let interface = file.interfaces().next().expect("one interface");
        match interface.members().next().expect("one member") {
            ast::InterfaceMember::Signal(signal) => signal.timing().expect("signal timing"),
            ast::InterfaceMember::Event(event) => event.timing().expect("event timing"),
            other => panic!("expected a signal or event, got {other:?}"),
        }
    }

    fn us(text: &str) -> ExactValue {
        ExactValue::parse(text).expect("a valid decimal")
    }

    fn codes(diags: &[Diagnostic]) -> Vec<&str> {
        diags.iter().map(|diag| diag.code.as_str()).collect()
    }

    /// [`resolve_timing`] with a throwaway file and anchor — the anchor is
    /// exercised through the checker, where a real interaction range exists.
    fn resolve(
        annot: Option<&ast::Timing>,
        kind: InteractionKind,
        default: &TimingSpec,
    ) -> (Option<TimingSpec>, Vec<Diagnostic>) {
        resolve_timing(
            annot,
            kind,
            default,
            file_id(),
            TextRange::empty(TextSize::from(0)),
        )
    }

    /// The exact microsecond value of a duration literal, ignoring whether it
    /// was written as a whole number.
    fn value_of(text: &str) -> Option<ExactValue> {
        duration_us(text).map(|duration| duration.us)
    }

    #[test]
    fn duration_conversions_are_exact_microseconds() {
        assert_eq!(value_of("10ms"), Some(us("10000")));
        assert_eq!(value_of("2s"), Some(us("2000000")));
        assert_eq!(value_of("100us"), Some(us("100")));
        assert_eq!(value_of("1s"), Some(us("1000000")));
        assert_eq!(value_of("5min"), Some(us("300000000")));
        assert_eq!(value_of("1h"), Some(us("3600000000")));
        assert!(value_of("fast").is_none());
        assert!(value_of("10").is_none(), "a bare number has no time unit");
    }

    #[test]
    fn fractional_durations_parse_exactly_but_are_flagged_not_whole() {
        // The lexer merges a FloatNumber with a time atom, so these reach the
        // module; the value is exact and `whole` marks the illegal form.
        let one_and_a_half_ms = duration_us("1.5ms").expect("parses");
        assert_eq!(one_and_a_half_ms.us, us("1500"));
        assert!(!one_and_a_half_ms.whole);

        let two_and_a_half_s = duration_us("2.5s").expect("parses");
        assert_eq!(two_and_a_half_s.us, us("2500000"));
        assert!(!two_and_a_half_s.whole);

        // A fractional microsecond stays exact rather than rounding.
        assert_eq!(duration_us("1.5us").expect("parses").us, us("1.5"));

        assert!(duration_us("10ms").expect("parses").whole);
    }

    #[test]
    fn strict_periodic_stores_the_period_in_both_bounds() {
        let (spec, diags) = resolve(
            Some(&annot("signal s : Speed @10ms")),
            InteractionKind::Signal,
            &builtin_default_timing(),
        );
        assert!(
            diags.is_empty(),
            "a valid strict period is clean: {diags:?}"
        );
        let spec = spec.expect("a signal always resolves timing");
        assert_eq!(spec.mode, TimingMode::StrictPeriodic);
        assert_eq!(spec.min_us, Some(us("10000")));
        assert_eq!(spec.max_us, Some(us("10000")));
        assert!(!spec.default_applied);
    }

    #[test]
    fn half_open_range_leaves_the_absent_side_unset() {
        let (spec, diags) = resolve(
            Some(&annot("signal s : Speed @[20ms..]")),
            InteractionKind::Signal,
            &builtin_default_timing(),
        );
        assert!(diags.is_empty(), "a half-open range is clean: {diags:?}");
        let spec = spec.expect("resolved");
        assert_eq!(spec.mode, TimingMode::Range);
        assert_eq!(spec.min_us, Some(us("20000")));
        assert_eq!(spec.max_us, None, "the absent upper bound stays unset");
    }

    /// Every timing message quotes the durations the author wrote, and none
    /// answers in the microseconds the IR canonicalises to.
    ///
    /// The assertion is on the written text, not merely on "some number":
    /// `@[100ms..50ms]` used to render `timing lower bound 100000us exceeds the
    /// upper bound 50000us`, which is a true statement about a unit that
    /// appears in no `.ridl` file. A message that regressed to microseconds
    /// would still name two bounds and still carry RIDL-101, so the code
    /// assertion alone cannot see it.
    fn assert_written_units(message: &str, quoted: &[&str]) {
        for text in quoted {
            assert!(
                message.contains(&format!("`{text}`")),
                "the message must quote the written duration `{text}`: {message}",
            );
        }
        // A digit followed by `us`, with or without a space between them, is a
        // microsecond count. Looking for the bare substring `us` would match
        // `because`; requiring the digit to be adjacent would miss `500000 us`.
        let micros = message.match_indices("us").any(|(at, _)| {
            message[..at]
                .trim_end_matches(' ')
                .ends_with(|c: char| c.is_ascii_digit())
        });
        assert!(
            !micros,
            "the message must not answer in canonical microseconds: {message}",
        );
    }

    #[test]
    fn ridl_101_lower_bound_exceeds_upper() {
        let (_, diags) = resolve(
            Some(&annot("signal s : Speed @[100ms..50ms]")),
            InteractionKind::Signal,
            &builtin_default_timing(),
        );
        assert_eq!(codes(&diags), vec!["RIDL-101"]);
        assert_written_units(&diags[0].message, &["100ms", "50ms"]);
        assert!(
            diags[0].message.contains("rate floor") && diags[0].message.contains("staleness bound"),
            "the message names both bounds by their role: {}",
            diags[0].message,
        );
    }

    #[test]
    fn ridl_102_zero_duration() {
        let (_, strict) = resolve(
            Some(&annot("signal s : Speed @0ms")),
            InteractionKind::Signal,
            &builtin_default_timing(),
        );
        assert_eq!(codes(&strict), vec!["RIDL-102"]);
        assert_written_units(&strict[0].message, &["0ms"]);
        assert!(
            strict[0].message.contains("strict period"),
            "the strict-period spelling is named as such: {}",
            strict[0].message,
        );

        let (_, ranged) = resolve(
            Some(&annot("signal s : Speed @[0ms..100ms]")),
            InteractionKind::Signal,
            &builtin_default_timing(),
        );
        assert_eq!(codes(&ranged), vec!["RIDL-102"]);
        assert_written_units(&ranged[0].message, &["0ms"]);
        assert!(
            ranged[0].message.contains("rate floor"),
            "the offending bound is named, not just the annotation: {}",
            ranged[0].message,
        );

        // The zero on the far side is reported against *its* role, so the
        // message never mislabels which bound the author must fix. The
        // inversion is reported too — `100ms > 0ms` — which is why the codes
        // are two here and not one.
        let (_, upper) = resolve(
            Some(&annot("signal s : Speed @[100ms..0ms]")),
            InteractionKind::Signal,
            &builtin_default_timing(),
        );
        assert_eq!(codes(&upper), vec!["RIDL-102", "RIDL-101"]);
        assert!(
            upper[0].message.contains("staleness bound") && upper[0].message.contains("`0ms`"),
            "the upper bound is the zero one here: {}",
            upper[0].message,
        );
    }

    #[test]
    fn ridl_103_strict_periodic_on_event() {
        let (spec, diags) = resolve(
            Some(&annot("event e : Speed @10ms")),
            InteractionKind::Event,
            &builtin_default_timing(),
        );
        assert_eq!(codes(&diags), vec!["RIDL-103"]);
        assert_written_units(&diags[0].message, &["@10ms"]);
        assert!(
            diags[0].message.contains("@[min..max]"),
            "the message names the spelling an event does take: {}",
            diags[0].message,
        );
        // The written bounds still lower honestly.
        assert_eq!(spec.expect("resolved").min_us, Some(us("10000")));
    }

    #[test]
    fn ridl_108_equal_range_bounds_warn() {
        let (_, diags) = resolve(
            Some(&annot("signal s : Speed @[30ms..30ms]")),
            InteractionKind::Signal,
            &builtin_default_timing(),
        );
        assert_eq!(codes(&diags), vec!["RIDL-108"]);
        assert_eq!(diags[0].severity, Severity::Warning);
        assert_written_units(&diags[0].message, &["30ms"]);
        // A signal is the one kind that has a strict period to offer, so it is
        // the one kind whose message offers it (ridl §9.2: a strict period is a
        // separate mode, admitted on signals only).
        assert!(
            diags[0].message.contains("`@30ms`"),
            "a signal is offered the strict period: {}",
            diags[0].message,
        );

        let (_, on_event) = resolve(
            Some(&annot("event e : Speed @[30ms..30ms]")),
            InteractionKind::Event,
            &builtin_default_timing(),
        );
        assert_eq!(codes(&on_event), vec!["RIDL-108"]);
        assert!(
            !on_event[0].message.contains("@30ms"),
            "an event has no strict period to be offered: {}",
            on_event[0].message,
        );
    }

    #[test]
    fn ridl_100_untimed_signal_applies_the_default_and_names_the_bounds() {
        let default = parse_default_timing("[50ms..2s]").expect("a valid default");
        let (spec, diags) = resolve(None, InteractionKind::Signal, &default);
        assert_eq!(codes(&diags), vec!["RIDL-100"]);
        assert_eq!(diags[0].severity, Severity::Warning);
        // The warning names the applied bounds as durations. Nothing was
        // written to echo here — the default comes from the manifest or the
        // built-in fallback — so the bounds are rendered in the units a
        // `[defaults].timing` is written in, never in canonical microseconds.
        assert_written_units(&diags[0].message, &[]);
        assert!(
            diags[0].message.contains("`@[50ms..2s]`"),
            "RIDL-100 must name the applied bounds as durations, got {:?}",
            diags[0].message,
        );
        let spec = spec.expect("a signal always resolves timing");
        assert!(spec.default_applied, "the applied default is flagged");
        assert_eq!(spec.min_us, Some(us("50000")));
        assert_eq!(spec.max_us, Some(us("2000000")));
    }

    /// [`render_duration`] picks the largest unit that divides the value
    /// exactly, so an applied default reads the way a manifest is written.
    #[test]
    fn durations_render_in_the_largest_exact_unit() {
        for (micros, rendered) in [
            ("1", "1us"),
            ("1500", "1500us"),
            ("100000", "100ms"),
            ("1000000", "1s"),
            ("1500000", "1500ms"),
            ("60000000", "1min"),
            ("3600000000", "1h"),
            ("7200000000", "2h"),
            // No unit divides a fractional microsecond, which only a rejected
            // literal (`@1.5us`) produces; it falls back rather than rounding.
            ("1.5", "1.5us"),
        ] {
            assert_eq!(render_duration(&us(micros)), rendered, "{micros}us");
        }
    }

    #[test]
    fn command_query_final_carry_no_timing() {
        for kind in [
            InteractionKind::Command,
            InteractionKind::Query,
            InteractionKind::Final,
        ] {
            let (spec, diags) = resolve(None, kind, &builtin_default_timing());
            assert_eq!(spec, None, "{kind:?} carries no timing");
            assert!(diags.is_empty(), "{kind:?} produces no timing diagnostic");
        }
    }

    #[test]
    fn parse_default_timing_reads_bounds_and_rejects_garbage() {
        let spec = parse_default_timing("[50ms..2s]").expect("valid");
        assert_eq!(spec.min_us, Some(us("50000")));
        assert_eq!(spec.max_us, Some(us("2000000")));
        assert!(!spec.default_applied);

        assert!(parse_default_timing("fast").is_err(), "not a range");
        assert!(
            parse_default_timing("[100ms..]").is_err(),
            "half-open default"
        );
        assert!(parse_default_timing("[0ms..1s]").is_err(), "zero bound");
        assert!(parse_default_timing("[2s..1s]").is_err(), "reversed");

        // A fractional bound is not a legal duration (ridl §2.1) — MANI-009.
        assert!(
            parse_default_timing("[1.5ms..100ms]").is_err(),
            "fractional lower bound",
        );
        assert!(
            parse_default_timing("[0.0ms..100ms]").is_err(),
            "fractional zero lower bound",
        );
    }

    // --- the T9 review regression: a written bound is never silently unset ---
    //
    // The lexer merges a FloatNumber with a time atom, so a fractional duration
    // reaches this module. It must draw a diagnostic AND still carry its exact
    // value: a dropped min/max is an invisible contract change for `ridl diff`,
    // and a strict period with both bounds unset is the shape ADR-0008 d12
    // forbids.

    #[test]
    fn fractional_strict_period_reports_and_keeps_its_bounds() {
        let (spec, diags) = resolve(
            Some(&annot("signal s : Speed @1.5ms")),
            InteractionKind::Signal,
            &builtin_default_timing(),
        );
        assert_eq!(codes(&diags), vec!["FORM-102"]);
        let spec = spec.expect("resolved");
        assert_eq!(spec.mode, TimingMode::StrictPeriodic);
        assert_eq!(spec.min_us, Some(us("1500")), "the period is carried");
        assert_eq!(spec.max_us, Some(us("1500")), "never left unset");
    }

    #[test]
    fn fractional_range_lower_bound_reports_and_keeps_the_rate_floor() {
        let (spec, diags) = resolve(
            Some(&annot("signal s : Speed @[1.5ms..100ms]")),
            InteractionKind::Signal,
            &builtin_default_timing(),
        );
        assert_eq!(codes(&diags), vec!["FORM-102"]);
        let spec = spec.expect("resolved");
        assert_eq!(
            spec.min_us,
            Some(us("1500")),
            "the rate floor must not be dropped",
        );
        assert_eq!(spec.max_us, Some(us("100000")));
    }

    #[test]
    fn fractional_range_upper_bound_reports_and_keeps_the_staleness_bound() {
        let (spec, diags) = resolve(
            Some(&annot("signal s : Speed @[20ms..2.5s]")),
            InteractionKind::Signal,
            &builtin_default_timing(),
        );
        assert_eq!(codes(&diags), vec!["FORM-102"]);
        let spec = spec.expect("resolved");
        assert_eq!(spec.min_us, Some(us("20000")));
        assert_eq!(
            spec.max_us,
            Some(us("2500000")),
            "the staleness bound must not be dropped",
        );
    }

    #[test]
    fn fractional_zero_duration_draws_both_form_102_and_ridl_102() {
        // `0.0ms` is both an illegal form and a zero value — it must not escape
        // the zero rule the way it did before the T9 review.
        let (spec, diags) = resolve(
            Some(&annot("signal s : Speed @0.0ms")),
            InteractionKind::Signal,
            &builtin_default_timing(),
        );
        assert!(
            codes(&diags).contains(&"RIDL-102"),
            "a zero duration must draw RIDL-102, got {:?}",
            codes(&diags),
        );
        assert!(
            codes(&diags).contains(&"FORM-102"),
            "a fractional literal must draw FORM-102, got {:?}",
            codes(&diags),
        );
        let spec = spec.expect("resolved");
        assert_eq!(spec.min_us, Some(us("0")), "the written zero is carried");
        assert_eq!(spec.max_us, Some(us("0")));
    }
}
