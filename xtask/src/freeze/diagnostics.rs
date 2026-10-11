//! The diagnostic catalogues: each code with its severity and, for a lint, its
//! name and default level. The summary text is not dumped.

use ridl_core::diag::{ALL_CATALOGS, Severity};
use ridl_core::lint::default_level;
use std::path::Path;

/// `<CODE> <severity>`, then ` lint=<name> default=<level>` for a lint.
pub fn dump(_root: &Path) -> Result<Vec<String>, String> {
    let mut lines = Vec::new();
    for (_, catalog) in ALL_CATALOGS {
        for entry in *catalog {
            let severity = match entry.severity {
                Severity::Error => "error",
                Severity::Warning => "warning",
                Severity::Info => "info",
            };
            let mut line = format!("{} {severity}", entry.code.as_str());
            if let Some(lint) = entry.lint {
                let level = default_level(entry)
                    .map(|l| l.as_str())
                    .ok_or_else(|| format!("{}: a lint without a default level", line))?;
                line.push_str(&format!(" lint={lint} default={level}"));
            }
            lines.push(line);
        }
    }
    Ok(lines)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostics_dump_carries_lint_and_level() {
        let lines = dump(Path::new(".")).unwrap();
        assert!(
            lines
                .iter()
                .any(|l| l.starts_with("RSDL-807 ") && l.contains("lint=link-encoding-not-emitted")),
            "{lines:?}"
        );
        let summary = ALL_CATALOGS
            .iter()
            .flat_map(|(_, c)| c.iter())
            .find(|e| e.code.as_str() == "RSDL-807")
            .unwrap()
            .summary;
        assert!(!summary.is_empty());
        assert!(lines.iter().all(|l| !l.contains(summary)));
    }
}
