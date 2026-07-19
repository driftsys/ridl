//! Timing resolution: parse and validate `@` annotations, apply the configured
//! default to untimed signals and events, and produce the resolved bounds the
//! IR carries (ridl language reference §9, general form §6.2, ADR-0008
//! decision 12; E2 task 9).
//!
//! Durations convert to exact microseconds — the three ridl §9 timing units
//! `us`/`ms`/`s` scale by 1 / 1_000 / 1_000_000; `min`/`h` (the remaining UCUM
//! atoms the lexer emits, ridl §2.8) scale by 60_000_000 / 3_600_000_000 —
//! always an exact integer count, because the lexer guarantees a duration has
//! no fractional part.
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
use rowan::{TextRange, TextSize};

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
    let min = duration_us(min_text).ok_or_else(|| format!("invalid duration `{min_text}`"))?;
    let max = duration_us(max_text).ok_or_else(|| format!("invalid duration `{max_text}`"))?;
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
/// applied, warning), RIDL-101 (lower bound exceeds upper), RIDL-102 (zero or
/// negative duration), RIDL-103 (strict `@Xms` on an event), RIDL-108 (`@[X..X]`
/// equal bounds, warning). An erroneous annotation still lowers its written
/// bounds, so the IR reflects the source honestly (the E1 discipline).
pub fn resolve_timing(
    annot: Option<&ast::Timing>,
    kind: InteractionKind,
    default: &TimingSpec,
    file: FileId,
) -> (Option<TimingSpec>, Vec<Diagnostic>) {
    if !matches!(kind, InteractionKind::Signal | InteractionKind::Event) {
        return (None, Vec::new());
    }

    let Some(annot) = annot else {
        // Untimed: the configured default is applied (ridl §9.1); RIDL-100
        // warns and names the applied bounds. The span-free signature leaves
        // the warning without a precise anchor, so it points at the file start
        // — the default is a package-level convenience, not a source token.
        let spec = applied_default(default);
        let diag = diagnostic(
            DiagCode::RIDL_100,
            Severity::Warning,
            file,
            TextRange::empty(TextSize::from(0)),
            format!(
                "{} without a timing annotation — the default {} is applied",
                kind_noun(kind),
                render_bounds(&spec),
            ),
        );
        return (Some(spec), vec![diag]);
    };

    let mut diags = Vec::new();
    if let Some(range) = annot.range() {
        // An explicit range `@[min..max]`, `@[min..]`, or `@[..max]`.
        let min = range.min().and_then(|token| duration_us(token.text()));
        let max = range.max().and_then(|token| duration_us(token.text()));
        let node = range.syntax().text_range();
        if min.as_ref().is_some_and(is_zero) || max.as_ref().is_some_and(is_zero) {
            diags.push(error(
                DiagCode::RIDL_102,
                file,
                node,
                "a timing duration must be greater than zero".to_string(),
            ));
        }
        if let (Some(lo), Some(hi)) = (&min, &max) {
            if lo > hi {
                diags.push(error(
                    DiagCode::RIDL_101,
                    file,
                    node,
                    format!(
                        "timing lower bound {}us exceeds the upper bound {}us",
                        lo.to_decimal_string(),
                        hi.to_decimal_string(),
                    ),
                ));
            } else if lo == hi {
                diags.push(diagnostic(
                    DiagCode::RIDL_108,
                    Severity::Warning,
                    file,
                    node,
                    format!(
                        "timing range with equal bounds {}us — equivalent to the strict period `@{}us`",
                        lo.to_decimal_string(),
                        lo.to_decimal_string(),
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
        // Strict periodic `@Xms` — signal only (ridl §9).
        let value = duration_us(token.text());
        let node = annot.syntax().text_range();
        if value.as_ref().is_some_and(is_zero) {
            diags.push(error(
                DiagCode::RIDL_102,
                file,
                node,
                "a timing duration must be greater than zero".to_string(),
            ));
        }
        if matches!(kind, InteractionKind::Event) {
            diags.push(error(
                DiagCode::RIDL_103,
                file,
                node,
                "strict-periodic `@Xms` is not valid on an event — use a range `@[min..max]`"
                    .to_string(),
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

/// The configured default, cloned and marked as applied (ridl §9.1).
fn applied_default(default: &TimingSpec) -> TimingSpec {
    TimingSpec {
        mode: TimingMode::Range,
        min_us: default.min_us.clone(),
        max_us: default.max_us.clone(),
        default_applied: true,
    }
}

/// Converts a duration literal (`"10ms"`, `"2s"`, `"100us"`, `"5min"`, `"1h"`)
/// to exact microseconds, or `None` when the text is not an integer followed by
/// a known UCUM time atom (ridl §2.8). Integer only: the lexer guarantees a
/// duration has no fractional part.
fn duration_us(text: &str) -> Option<ExactValue> {
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
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let value = BigInt::parse_bytes(digits.as_bytes(), 10)?;
    let micros = value * BigInt::from(factor);
    Some(ExactValue(BigRational::from_integer(micros)))
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

/// Renders a resolved timing as `[<min>us..<max>us]` for a diagnostic message —
/// each bound in exact microseconds, an unset bound rendered as an empty side.
fn render_bounds(spec: &TimingSpec) -> String {
    let render = |bound: &Option<ExactValue>| match bound {
        Some(value) => format!("{}us", value.to_decimal_string()),
        None => String::new(),
    };
    format!("[{}..{}]", render(&spec.min_us), render(&spec.max_us))
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

    #[test]
    fn duration_conversions_are_exact_microseconds() {
        assert_eq!(duration_us("10ms"), Some(us("10000")));
        assert_eq!(duration_us("2s"), Some(us("2000000")));
        assert_eq!(duration_us("100us"), Some(us("100")));
        assert_eq!(duration_us("1s"), Some(us("1000000")));
        assert_eq!(duration_us("5min"), Some(us("300000000")));
        assert_eq!(duration_us("1h"), Some(us("3600000000")));
        assert_eq!(duration_us("fast"), None);
        assert_eq!(duration_us("10"), None);
    }

    #[test]
    fn strict_periodic_stores_the_period_in_both_bounds() {
        let (spec, diags) = resolve_timing(
            Some(&annot("signal s : Speed @10ms")),
            InteractionKind::Signal,
            &builtin_default_timing(),
            file_id(),
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
        let (spec, diags) = resolve_timing(
            Some(&annot("signal s : Speed @[20ms..]")),
            InteractionKind::Signal,
            &builtin_default_timing(),
            file_id(),
        );
        assert!(diags.is_empty(), "a half-open range is clean: {diags:?}");
        let spec = spec.expect("resolved");
        assert_eq!(spec.mode, TimingMode::Range);
        assert_eq!(spec.min_us, Some(us("20000")));
        assert_eq!(spec.max_us, None, "the absent upper bound stays unset");
    }

    #[test]
    fn ridl_101_lower_bound_exceeds_upper() {
        let (_, diags) = resolve_timing(
            Some(&annot("signal s : Speed @[100ms..50ms]")),
            InteractionKind::Signal,
            &builtin_default_timing(),
            file_id(),
        );
        assert_eq!(codes(&diags), vec!["RIDL-101"]);
    }

    #[test]
    fn ridl_102_zero_duration() {
        let (_, strict) = resolve_timing(
            Some(&annot("signal s : Speed @0ms")),
            InteractionKind::Signal,
            &builtin_default_timing(),
            file_id(),
        );
        assert_eq!(codes(&strict), vec!["RIDL-102"]);

        let (_, ranged) = resolve_timing(
            Some(&annot("signal s : Speed @[0ms..100ms]")),
            InteractionKind::Signal,
            &builtin_default_timing(),
            file_id(),
        );
        assert_eq!(codes(&ranged), vec!["RIDL-102"]);
    }

    #[test]
    fn ridl_103_strict_periodic_on_event() {
        let (spec, diags) = resolve_timing(
            Some(&annot("event e : Speed @10ms")),
            InteractionKind::Event,
            &builtin_default_timing(),
            file_id(),
        );
        assert_eq!(codes(&diags), vec!["RIDL-103"]);
        // The written bounds still lower honestly.
        assert_eq!(spec.expect("resolved").min_us, Some(us("10000")));
    }

    #[test]
    fn ridl_108_equal_range_bounds_warn() {
        let (_, diags) = resolve_timing(
            Some(&annot("signal s : Speed @[30ms..30ms]")),
            InteractionKind::Signal,
            &builtin_default_timing(),
            file_id(),
        );
        assert_eq!(codes(&diags), vec!["RIDL-108"]);
        assert_eq!(diags[0].severity, Severity::Warning);
    }

    #[test]
    fn ridl_100_untimed_signal_applies_the_default_and_names_the_bounds() {
        let default = parse_default_timing("[50ms..2s]").expect("a valid default");
        let (spec, diags) = resolve_timing(None, InteractionKind::Signal, &default, file_id());
        assert_eq!(codes(&diags), vec!["RIDL-100"]);
        assert_eq!(diags[0].severity, Severity::Warning);
        // The warning names the applied bounds in microseconds.
        assert!(
            diags[0].message.contains("50000") && diags[0].message.contains("2000000"),
            "RIDL-100 must name the applied bounds, got {:?}",
            diags[0].message,
        );
        let spec = spec.expect("a signal always resolves timing");
        assert!(spec.default_applied, "the applied default is flagged");
        assert_eq!(spec.min_us, Some(us("50000")));
        assert_eq!(spec.max_us, Some(us("2000000")));
    }

    #[test]
    fn command_query_final_carry_no_timing() {
        for kind in [
            InteractionKind::Command,
            InteractionKind::Query,
            InteractionKind::Final,
        ] {
            let (spec, diags) = resolve_timing(None, kind, &builtin_default_timing(), file_id());
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
    }
}
