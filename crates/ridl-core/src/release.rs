//! Workspace release pins and numeric release compatibility.
use crate::diag::{DiagCode, Diagnostic, FileId, Severity, Span};
use rowan::{TextRange, TextSize};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::{fmt, ops::Range};
use toml::Spanned;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Release {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
}
impl fmt::Display for Release {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}
pub const MIN_PINNABLE_RELEASE: Release = Release {
    major: 0,
    minor: 23,
    patch: 0,
};
/// Parses exactly three nonempty ASCII decimal components that fit `u64`.
pub fn parse_release(text: &str) -> Option<Release> {
    let mut parts = text.split('.');
    let mut next = || {
        let part = parts.next()?;
        if part.is_empty() || !part.bytes().all(|c| c.is_ascii_digit()) {
            return None;
        }
        part.parse().ok()
    };
    let release = Release {
        major: next()?,
        minor: next()?,
        patch: next()?,
    };
    if parts.next().is_some() {
        return None;
    }
    Some(release)
}
/// A pin accepts the same major and minor and any patch at least its own.
pub fn compatible(running: Release, pin: Release) -> bool {
    running.major == pin.major && running.minor == pin.minor && running.patch >= pin.patch
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PinValue {
    pub text: Option<String>,
    pub span: Range<usize>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleasePin {
    pub release: Release,
    pub span: Range<usize>,
}
/// Reads pin presence and its original value span without typed manifest validation.
/// Broken TOML has no pin; the manifest parser reports MANI-001 instead.
pub fn pin_value(text: &str) -> Option<PinValue> {
    // Validate syntax without imposing either mode's schema. Deserialize only
    // the selected mode for the nested value span, ignoring the other mode's shape.
    let doc: BTreeMap<String, Spanned<toml::Value>> = toml::from_str(text).ok()?;
    #[derive(Deserialize)]
    struct WorkspacePin {
        workspace: BTreeMap<String, Spanned<toml::Value>>,
    }
    #[derive(Deserialize)]
    struct PackagePin {
        package: BTreeMap<String, Spanned<toml::Value>>,
    }
    let has_workspace_pin = doc
        .get("workspace")
        .and_then(|v| v.get_ref().as_table())
        .is_some_and(|t| t.contains_key("ridl-version"));
    let mut table = if has_workspace_pin {
        toml::from_str::<WorkspacePin>(text).ok()?.workspace
    } else {
        toml::from_str::<PackagePin>(text).ok()?.package
    };
    let value = table.remove("ridl-version")?;
    Some(PinValue {
        text: value.get_ref().as_str().map(str::to_string),
        span: value.span(),
    })
}

/// Validates a standalone pin's grammar and floor, without comparing a running release.
pub fn parse_release_pin(file: FileId, text: &str) -> Result<Option<ReleasePin>, Diagnostic> {
    let Some(value) = pin_value(text) else {
        return Ok(None);
    };
    let release = value
        .text
        .as_deref()
        .and_then(parse_release)
        .ok_or_else(|| {
            pin_diagnostic(
                DiagCode::MANI_024,
                file,
                value.span.clone(),
                "`ridl-version` must be a string of the form MAJOR.MINOR.PATCH".to_string(),
            )
        })?;
    if release < MIN_PINNABLE_RELEASE {
        return Err(pin_diagnostic(
            DiagCode::MANI_025,
            file,
            value.span,
            format!(
                "`ridl-version` {release} is below the minimum pinnable release {MIN_PINNABLE_RELEASE}"
            ),
        ));
    }
    Ok(Some(ReleasePin {
        release,
        span: value.span,
    }))
}

/// The single refusal that takes priority over every other workspace diagnostic.
pub fn check_release_pin(file: FileId, text: &str, running: Release) -> Option<Diagnostic> {
    match parse_release_pin(file, text) {
        Err(diagnostic) => Some(diagnostic),
        Ok(Some(pin)) if !compatible(running, pin.release) => Some(pin_diagnostic(
            DiagCode::MANI_026,
            file,
            pin.span,
            format!(
                "workspace pins ridl {}, but the running release is {running}; run `ridl toolchain install {}`",
                pin.release, pin.release
            ),
        )),
        _ => None,
    }
}

/// Identifies release refusals by code even if a caller changed their severity.
pub fn is_release_refusal(diagnostic: &Diagnostic) -> bool {
    matches!(
        diagnostic.code,
        DiagCode::MANI_024 | DiagCode::MANI_025 | DiagCode::MANI_026
    )
}

pub(crate) fn pin_diagnostic(
    code: DiagCode,
    file: FileId,
    range: Range<usize>,
    message: String,
) -> Diagnostic {
    Diagnostic {
        code,
        severity: if code == DiagCode::MANI_027 {
            Severity::Warning
        } else {
            Severity::Error
        },
        primary: Span {
            file,
            range: TextRange::new(
                TextSize::from(range.start as u32),
                TextSize::from(range.end as u32),
            ),
        },
        message,
        labels: Vec::new(),
        fixits: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn release_pin_accepts_only_three_decimal_components() {
        let values = [
            "0.23.0",
            "1.2.3",
            "00.23.0",
            "v0.23.0",
            "editor-v0.23.0",
            "0.23",
            "0.23.0-rc.1",
            "0.23.0+1",
            " 0.23.0",
            "-1.23.0",
            "1.2.18446744073709551616",
            "1.2.3.4",
            "1..3",
            "１.2.3",
            "+1.2.3",
        ];
        let accepted: Vec<_> = values
            .iter()
            .copied()
            .filter(|v| parse_release(v).is_some())
            .collect();
        assert_eq!(accepted, ["0.23.0", "1.2.3", "00.23.0"]);
        assert_eq!(parse_release("00.23.0").unwrap().to_string(), "0.23.0");
    }
    #[test]
    fn release_compatibility_uses_major_minor_and_patch() {
        let pin = Release {
            major: 0,
            minor: 23,
            patch: 2,
        };
        let values = ["0.23.2", "0.23.4", "0.23.1", "0.22.9", "0.24.0", "1.23.2"];
        let accepted: Vec<_> = values
            .iter()
            .copied()
            .filter(|v| {
                let c: Vec<u64> = v.split('.').map(|c| c.parse().unwrap()).collect();
                compatible(
                    Release {
                        major: c[0],
                        minor: c[1],
                        patch: c[2],
                    },
                    pin,
                )
            })
            .collect();
        assert_eq!(accepted, ["0.23.2", "0.23.4"]);
        assert!(
            Release {
                major: 1,
                minor: 0,
                patch: 0
            } > MIN_PINNABLE_RELEASE
        );
    }
    #[test]
    fn release_pin_reports_value_spans_and_floor() {
        for mode in ["package", "workspace"] {
            for (value, code) in [
                ("42", "MANI-024"),
                ("\"bad\"", "MANI-024"),
                ("\"0.22.0\"", "MANI-025"),
                ("\"0.23.0\"", "MANI-026"),
            ] {
                let text = format!("[{mode}]\nridl-version = {value}\n");
                let d = check_release_pin(
                    FileId::DETACHED,
                    &text,
                    Release {
                        major: 0,
                        minor: 24,
                        patch: 0,
                    },
                )
                .expect("pin diagnostic");
                assert_eq!(d.code.as_str(), code);
                assert_eq!(
                    &text[usize::from(d.primary.range.start())..usize::from(d.primary.range.end())],
                    value
                );
                assert!(is_release_refusal(&d));
                if code == "MANI-026" {
                    for part in ["0.23.0", "0.24.0", "ridl toolchain install 0.23.0"] {
                        assert!(d.message.contains(part), "{}", d.message);
                    }
                }
            }
            let text = format!("[{mode}]\nridl-version = \"0.23.0\"\n");
            let pin = parse_release_pin(FileId::DETACHED, &text).unwrap().unwrap();
            assert_eq!(&text[pin.span], "\"0.23.0\"");
            assert!(
                check_release_pin(
                    FileId::DETACHED,
                    &text,
                    Release {
                        major: 0,
                        minor: 23,
                        patch: 2
                    }
                )
                .is_none()
            );
        }
    }
    #[test]
    fn release_workspace_pin_takes_precedence_and_refusals_ignore_severity() {
        let running = Release {
            major: 0,
            minor: 23,
            patch: 2,
        };
        let text = "[workspace]\nridl-version = \"0.24.0\"\n[package]\nridl-version = \"0.23.0\"\n";
        assert_eq!(
            check_release_pin(FileId::DETACHED, text, running)
                .unwrap()
                .code
                .as_str(),
            "MANI-026"
        );
        let fallback = "[workspace]\n[package]\nridl-version = 42\n";
        let mut diagnostic = check_release_pin(FileId::DETACHED, fallback, running).unwrap();
        diagnostic.severity = Severity::Warning;
        assert!(is_release_refusal(&diagnostic));
        diagnostic.code = DiagCode::MANI_027;
        diagnostic.severity = Severity::Error;
        assert!(!is_release_refusal(&diagnostic));
        assert!(pin_value("[workspace]\nridl-version = [").is_none());
    }
    #[test]
    fn release_prepass_ignores_unrelated_mode_shape_errors() {
        for text in [
            "package = 42\n[workspace]\nridl-version = \"0.24.0\"\n",
            "workspace = 42\n[package]\nridl-version = \"0.24.0\"\n",
            "[package]\nname = 42\nridl-version = \"0.24.0\"\n",
        ] {
            let diagnostic = check_release_pin(FileId::DETACHED, text, MIN_PINNABLE_RELEASE)
                .expect("pin precedes unrelated schema errors");
            assert_eq!(diagnostic.code, DiagCode::MANI_026);
        }
    }
}
